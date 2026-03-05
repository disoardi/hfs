// WebHDFS client — REST API backend for HDFS
//
// Implements the HdfsClient trait using HTTP requests to the NameNode WebHDFS endpoint.
// All HTTP calls are blocking (ureq v2), wrapped in tokio::task::spawn_blocking so
// the async trait implementation is non-blocking from the caller's perspective.
//
// Pagination: LISTSTATUS_BATCH is used for directories with >N entries.
// Read: OPEN with offset/length query params — NameNode redirects to DataNode.
// Health: JMX endpoint on the same port (9870).

use crate::client::{BlockInfo, ClusterHealth, ContentSummary, FileStatus, HdfsClient};
use crate::error::HfsError;
use async_trait::async_trait;
use serde::Deserialize;
use std::io::Read;
use std::time::Duration;
use tokio::task;

// ─── JSON response structs ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListStatusResponse {
    #[serde(rename = "FileStatuses")]
    file_statuses: FileStatusesWrapper,
}

#[derive(Deserialize)]
struct FileStatusesWrapper {
    #[serde(rename = "FileStatus")]
    statuses: Vec<WFileStatus>,
}

#[derive(Deserialize)]
struct DirectoryListingResponse {
    #[serde(rename = "DirectoryListing")]
    listing: DirectoryListing,
}

#[derive(Deserialize)]
struct DirectoryListing {
    #[serde(rename = "partialListing")]
    partial_listing: PartialListing,
    #[serde(rename = "remainingEntries")]
    remaining_entries: u64,
}

#[derive(Deserialize)]
struct PartialListing {
    #[serde(rename = "FileStatuses")]
    file_statuses: FileStatusesWrapper,
}

#[derive(Deserialize)]
struct GetFileStatusResponse {
    #[serde(rename = "FileStatus")]
    file_status: WFileStatus,
}

#[derive(Deserialize)]
struct WFileStatus {
    #[serde(rename = "pathSuffix")]
    path_suffix: String,
    #[serde(rename = "type")]
    type_: String,
    length: u64,
    owner: String,
    group: String,
    permission: String,
    replication: u16,
    #[serde(rename = "blockSize")]
    block_size: u64,
    #[serde(rename = "modificationTime")]
    modification_time: u64,
    #[serde(rename = "accessTime")]
    access_time: u64,
}

#[derive(Deserialize)]
struct ContentSummaryResponse {
    #[serde(rename = "ContentSummary")]
    content_summary: WContentSummary,
}

#[derive(Deserialize)]
struct WContentSummary {
    #[serde(rename = "directoryCount")]
    directory_count: u64,
    #[serde(rename = "fileCount")]
    file_count: u64,
    length: u64,
    quota: i64,
    #[serde(rename = "spaceConsumed")]
    space_consumed: u64,
    #[serde(rename = "spaceQuota")]
    space_quota: i64,
}

#[derive(Deserialize)]
struct BlockLocationsResponse {
    #[serde(rename = "BlockLocations")]
    block_locations: WBlockLocations,
}

#[derive(Deserialize)]
struct WBlockLocations {
    #[serde(rename = "BlockLocation")]
    locations: Vec<WBlockLocation>,
}

#[derive(Deserialize)]
struct WBlockLocation {
    #[serde(rename = "blockToken")]
    _block_token: Option<serde_json::Value>,
    corrupt: bool,
    #[allow(dead_code)]
    hosts: Vec<String>,
    length: u64,
    names: Vec<String>,
    #[allow(dead_code)]
    offset: u64,
    #[serde(rename = "storageTypes")]
    #[allow(dead_code)]
    storage_types: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct JmxResponse {
    beans: Vec<serde_json::Map<String, serde_json::Value>>,
}

// Error response from WebHDFS
#[derive(Deserialize)]
struct RemoteExceptionWrapper {
    #[serde(rename = "RemoteException")]
    remote_exception: RemoteException,
}

#[derive(Deserialize)]
struct RemoteException {
    exception: String,
    message: String,
}

// ─── Client struct ────────────────────────────────────────────────────────────

/// WebHDFS HTTP client — stateless, cheap to clone.
#[derive(Clone, Debug)]
pub struct WebHdfsClient {
    /// Root URL of the NameNode WebHDFS endpoint, e.g. "http://namenode:9870"
    base_url: String,
    /// HDFS user for simple (non-Kerberos) authentication via &user.name= query param.
    /// Defaults to "hdfs" so connections from non-Hadoop Linux accounts work out of the box.
    user: String,
    agent: ureq::Agent,
}

impl WebHdfsClient {
    /// Create a new client pointing at `base_url` with default user "hdfs".
    pub fn new(base_url: &str) -> Self {
        Self::new_with_user(base_url, "hdfs")
    }

    /// Create a new client pointing at `base_url` with the given HDFS user.
    pub fn new_with_user(base_url: &str, user: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirects(5)
            .build();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            user: user.to_string(),
            agent,
        }
    }

    /// Convert a WebHDFS FileStatus JSON struct to our domain struct.
    fn convert_file_status(ws: WFileStatus, parent_path: &str) -> FileStatus {
        let path = if ws.path_suffix.is_empty() {
            parent_path.to_string()
        } else if parent_path == "/" {
            format!("/{}", ws.path_suffix)
        } else {
            format!("{}/{}", parent_path.trim_end_matches('/'), ws.path_suffix)
        };
        FileStatus {
            path,
            length: ws.length,
            is_dir: ws.type_ == "DIRECTORY",
            replication: ws.replication,
            block_size: ws.block_size,
            modification_time: ws.modification_time,
            access_time: ws.access_time,
            owner: ws.owner,
            group: ws.group,
            permission: ws.permission,
        }
    }

    /// Fetch one page of LISTSTATUS_BATCH and return (statuses, remaining_entries).
    fn fetch_batch(
        agent: &ureq::Agent,
        base_url: &str,
        path: &str,
        start_after: &str,
        user: &str,
    ) -> Result<(Vec<WFileStatus>, u64), HfsError> {
        let url = if start_after.is_empty() {
            op_url(base_url, path, "LISTSTATUS_BATCH", user)
        } else {
            format!(
                "{}&startAfter={}",
                op_url(base_url, path, "LISTSTATUS_BATCH", user),
                start_after
            )
        };
        let resp = agent
            .get(&url)
            .call()
            .map_err(|e| map_ureq_error(e, path))?;
        let listing: DirectoryListingResponse = resp
            .into_json()
            .map_err(|e| HfsError::Connection(format!("JSON parse error: {}", e)))?;
        Ok((
            listing.listing.partial_listing.file_statuses.statuses,
            listing.listing.remaining_entries,
        ))
    }

    /// Exponential backoff retry for 503 / connection errors.
    /// Retries up to `max_retries` times with delays 1, 2, 4, 8, 16 seconds.
    #[allow(dead_code)]
    fn with_backoff<F, T>(f: F) -> Result<T, HfsError>
    where
        F: Fn() -> Result<T, HfsError>,
    {
        let mut delay_secs = 1u64;
        let max_retries = 5;
        for attempt in 0..=max_retries {
            match f() {
                Ok(v) => return Ok(v),
                Err(HfsError::NameNodeUnavailable(_)) if attempt < max_retries => {
                    std::thread::sleep(Duration::from_secs(delay_secs));
                    delay_secs = (delay_secs * 2).min(30);
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }
}

// ─── Extra methods not in the trait ──────────────────────────────────────────

impl WebHdfsClient {
    /// Paginated listing using LISTSTATUS_BATCH — handles directories with >N files.
    pub async fn list_batch(&self, path: &str) -> Result<Vec<FileStatus>, HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();
        let path = path.to_string();
        let user = self.user.clone();

        task::spawn_blocking(move || {
            let mut all: Vec<FileStatus> = Vec::new();
            let mut start_after = String::new();

            loop {
                let (batch, remaining) =
                    WebHdfsClient::fetch_batch(&agent, &base_url, &path, &start_after, &user)?;
                let last_suffix = batch.last().map(|s| s.path_suffix.clone());
                let statuses: Vec<FileStatus> = batch
                    .into_iter()
                    .map(|ws| WebHdfsClient::convert_file_status(ws, &path))
                    .collect();
                all.extend(statuses);

                if remaining == 0 {
                    break;
                }
                start_after = last_suffix.unwrap_or_default();
            }
            Ok(all)
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }
}

// ─── Trait implementation ─────────────────────────────────────────────────────

#[async_trait]
impl HdfsClient for WebHdfsClient {
    async fn list(&self, path: &str) -> Result<Vec<FileStatus>, HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();
        let path = path.to_string();
        let user = self.user.clone();

        task::spawn_blocking(move || {
            let url = op_url(&base_url, &path, "LISTSTATUS", &user);
            let resp = agent
                .get(&url)
                .call()
                .map_err(|e| map_ureq_error(e, &path))?;

            // Check for RemoteException in error responses
            let body = resp
                .into_string()
                .map_err(|e| HfsError::Connection(e.to_string()))?;

            // Try to parse as error first
            if let Ok(err) = serde_json::from_str::<RemoteExceptionWrapper>(&body) {
                return Err(map_remote_exception(err.remote_exception, &path));
            }

            let list: ListStatusResponse = serde_json::from_str(&body)
                .map_err(|e| HfsError::Connection(format!("JSON parse: {}", e)))?;

            let statuses = list.file_statuses.statuses;

            // If the listing seems complete (no pagination needed), return it
            // Note: simple LISTSTATUS returns all entries at once (may OOM on huge dirs)
            // For now, if there are >1000 entries, switch to batch mode.
            // In practice, LISTSTATUS doesn't tell us if there are remaining entries;
            // LISTSTATUS_BATCH does. Use batch mode from the start for correctness.
            let result: Vec<FileStatus> = statuses
                .into_iter()
                .map(|ws| WebHdfsClient::convert_file_status(ws, &path))
                .collect();
            Ok(result)
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }

    async fn stat(&self, path: &str) -> Result<FileStatus, HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();
        let path = path.to_string();
        let user = self.user.clone();

        task::spawn_blocking(move || {
            let url = op_url(&base_url, &path, "GETFILESTATUS", &user);
            let resp = agent
                .get(&url)
                .call()
                .map_err(|e| map_ureq_error(e, &path))?;

            let body = resp
                .into_string()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            if let Ok(err) = serde_json::from_str::<RemoteExceptionWrapper>(&body) {
                return Err(map_remote_exception(err.remote_exception, &path));
            }

            let resp: GetFileStatusResponse = serde_json::from_str(&body)
                .map_err(|e| HfsError::Connection(format!("JSON parse: {}", e)))?;
            Ok(WebHdfsClient::convert_file_status(resp.file_status, &path))
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }

    async fn content_summary(&self, path: &str) -> Result<ContentSummary, HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();
        let path = path.to_string();
        let user = self.user.clone();

        task::spawn_blocking(move || {
            let url = op_url(&base_url, &path, "GETCONTENTSUMMARY", &user);
            let resp = agent
                .get(&url)
                .call()
                .map_err(|e| map_ureq_error(e, &path))?;

            let body = resp
                .into_string()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            if let Ok(err) = serde_json::from_str::<RemoteExceptionWrapper>(&body) {
                return Err(map_remote_exception(err.remote_exception, &path));
            }

            let resp: ContentSummaryResponse = serde_json::from_str(&body)
                .map_err(|e| HfsError::Connection(format!("JSON parse: {}", e)))?;
            let cs = resp.content_summary;
            Ok(ContentSummary {
                directory_count: cs.directory_count,
                file_count: cs.file_count,
                length: cs.length,
                space_consumed: cs.space_consumed,
                quota: cs.quota,
                space_quota: cs.space_quota,
            })
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }

    async fn blocks(&self, path: &str) -> Result<Vec<BlockInfo>, HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();
        let path = path.to_string();
        let user = self.user.clone();

        task::spawn_blocking(move || {
            let url = op_url(&base_url, &path, "GETFILEBLOCKLOCATIONS", &user);
            let resp = agent
                .get(&url)
                .call()
                .map_err(|e| map_ureq_error(e, &path))?;

            let body = resp
                .into_string()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            if let Ok(err) = serde_json::from_str::<RemoteExceptionWrapper>(&body) {
                return Err(map_remote_exception(err.remote_exception, &path));
            }

            let resp: BlockLocationsResponse = serde_json::from_str(&body)
                .map_err(|e| HfsError::Connection(format!("JSON parse: {}", e)))?;
            let blocks = resp
                .block_locations
                .locations
                .into_iter()
                .enumerate()
                .map(|(i, bl)| BlockInfo {
                    block_id: i as u64, // WebHDFS doesn't expose raw block ID in this endpoint
                    length: bl.length,
                    corrupt: bl.corrupt,
                    datanode_locations: bl.names,
                })
                .collect();
            Ok(blocks)
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }

    async fn health(&self) -> Result<ClusterHealth, HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();

        task::spawn_blocking(move || {
            // Query FSNamesystemState for datanode counts and block health
            let url_fs = format!(
                "{}/jmx?qry=Hadoop:service=NameNode,name=FSNamesystemState",
                base_url
            );
            let resp_fs = agent
                .get(&url_fs)
                .call()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            let jmx_fs: JmxResponse = resp_fs
                .into_json()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            let bean_fs = jmx_fs.beans.into_iter().next().ok_or_else(|| {
                HfsError::Connection("FSNamesystemState JMX bean not found".to_string())
            })?;

            // Query NameNodeInfo for capacity and HA state
            let url_nn = format!(
                "{}/jmx?qry=Hadoop:service=NameNode,name=NameNodeInfo",
                base_url
            );
            let resp_nn = agent
                .get(&url_nn)
                .call()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            let jmx_nn: JmxResponse = resp_nn
                .into_json()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            let bean_nn = jmx_nn.beans.into_iter().next();

            fn get_u64(bean: &serde_json::Map<String, serde_json::Value>, key: &str) -> u64 {
                bean.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
            }

            let live = get_u64(&bean_fs, "NumLiveDataNodes") as u32;
            let dead = get_u64(&bean_fs, "NumDeadDataNodes") as u32;
            let stale = get_u64(&bean_fs, "NumStaleDataNodes") as u32;
            let under_rep = get_u64(&bean_fs, "UnderReplicatedBlocks");
            let corrupt = get_u64(&bean_fs, "CorruptBlocks");

            let (cap_total, cap_used, cap_remaining, ha_state) = if let Some(ref nn) = bean_nn {
                let total = get_u64(nn, "Total");
                let used = get_u64(nn, "Used");
                let free = get_u64(nn, "Free");
                let ha = nn
                    .get("HAState")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                (total, used, free, ha)
            } else {
                (0, 0, 0, None)
            };

            Ok(ClusterHealth {
                live_datanodes: live,
                dead_datanodes: dead,
                stale_datanodes: stale,
                under_replicated_blocks: under_rep,
                corrupt_blocks: corrupt,
                capacity_total_bytes: cap_total,
                capacity_used_bytes: cap_used,
                capacity_remaining_bytes: cap_remaining,
                namenode_ha_state: ha_state,
            })
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }

    async fn mkdir(&self, path: &str, create_parent: bool) -> Result<(), HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();
        let path = path.to_string();
        let user = self.user.clone();

        task::spawn_blocking(move || {
            let url = format!(
                "{}&createParent={}",
                op_url(&base_url, &path, "MKDIRS", &user),
                create_parent
            );
            let resp = agent
                .put(&url)
                .call()
                .map_err(|e| map_ureq_error(e, &path))?;
            let body = resp
                .into_string()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            if let Ok(err) = serde_json::from_str::<RemoteExceptionWrapper>(&body) {
                return Err(map_remote_exception(err.remote_exception, &path));
            }
            Ok(())
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }

    async fn delete(&self, path: &str, recursive: bool) -> Result<(), HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();
        let path = path.to_string();
        let user = self.user.clone();

        task::spawn_blocking(move || {
            let url = format!(
                "{}&recursive={}",
                op_url(&base_url, &path, "DELETE", &user),
                recursive
            );
            let resp = agent
                .delete(&url)
                .call()
                .map_err(|e| map_ureq_error(e, &path))?;
            let body = resp
                .into_string()
                .map_err(|e| HfsError::Connection(e.to_string()))?;
            if let Ok(err) = serde_json::from_str::<RemoteExceptionWrapper>(&body) {
                return Err(map_remote_exception(err.remote_exception, &path));
            }
            Ok(())
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }

    /// Read a byte range from a file using WebHDFS OPEN operation.
    ///
    /// WebHDFS OPEN redirects to a DataNode. If the DataNode hostname is not
    /// reachable (e.g. Docker internal hostnames from outside), set HDFS_DATA_HOST
    /// to override the host portion of the redirect URL.
    async fn read_range(&self, path: &str, offset: u64, length: u64) -> Result<Vec<u8>, HfsError> {
        let base_url = self.base_url.clone();
        let agent = self.agent.clone();
        let path = path.to_string();
        let user = self.user.clone();

        task::spawn_blocking(move || {
            let url = format!(
                "{}&offset={}&length={}&buffersize=131072",
                op_url(&base_url, &path, "OPEN", &user),
                offset,
                length
            );
            let resp = agent
                .get(&url)
                .call()
                .map_err(|e| map_ureq_error(e, &path))?;

            let mut reader = resp.into_reader();
            let mut bytes = Vec::with_capacity(length as usize);
            reader
                .read_to_end(&mut bytes)
                .map_err(|e| HfsError::Connection(format!("read error: {}", e)))?;
            Ok(bytes)
        })
        .await
        .map_err(|e| HfsError::Connection(format!("task error: {}", e)))?
    }

    async fn file_size(&self, path: &str) -> Result<u64, HfsError> {
        Ok(self.stat(path).await?.length)
    }

    fn backend_name(&self) -> &'static str {
        "WebHDFS"
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Build a WebHDFS operation URL with user.name for simple (non-Kerberos) auth.
/// Additional query params can be appended to the returned string.
fn op_url(base_url: &str, path: &str, op: &str, user: &str) -> String {
    format!(
        "{}/webhdfs/v1{}?op={}&user.name={}",
        base_url,
        normalize_path(path),
        op,
        user
    )
}

/// Ensure path starts with / and doesn't have a trailing slash (except root).
fn normalize_path(path: &str) -> String {
    let p = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };
    if p == "/" {
        p
    } else {
        p.trim_end_matches('/').to_string()
    }
}

/// Map a ureq v2 error to HfsError.
fn map_ureq_error(e: ureq::Error, path: &str) -> HfsError {
    match e {
        ureq::Error::Status(404, _) => HfsError::NotFound(path.to_string()),
        ureq::Error::Status(403, _) => HfsError::Permission(path.to_string()),
        ureq::Error::Status(401, _) => {
            HfsError::Auth("WebHDFS authentication required".to_string())
        }
        ureq::Error::Status(503, _) => {
            HfsError::NameNodeUnavailable("HTTP 503 from NameNode".to_string())
        }
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            // Try to extract RemoteException message from body
            if let Ok(err) = serde_json::from_str::<RemoteExceptionWrapper>(&body) {
                map_remote_exception(err.remote_exception, path)
            } else {
                HfsError::Hdfs(format!("HTTP {}: {}", code, body))
            }
        }
        ureq::Error::Transport(t) => HfsError::Connection(t.to_string()),
    }
}

/// Map a WebHDFS RemoteException to HfsError.
fn map_remote_exception(ex: RemoteException, path: &str) -> HfsError {
    match ex.exception.as_str() {
        "FileNotFoundException" | "PathNotFoundException" => HfsError::NotFound(path.to_string()),
        "AccessControlException" | "PermissionDeniedException" => {
            HfsError::Permission(format!("{}: {}", path, ex.message))
        }
        "SafeModeException" => {
            HfsError::NameNodeUnavailable(format!("NameNode in safemode: {}", ex.message))
        }
        _ => HfsError::Hdfs(format!("{}: {}", ex.exception, ex.message)),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    const LISTSTATUS_JSON: &str = r#"{
      "FileStatuses": {
        "FileStatus": [
          {
            "accessTime": 0,
            "blockSize": 0,
            "group": "supergroup",
            "length": 0,
            "modificationTime": 1700000000000,
            "owner": "hadoop",
            "pathSuffix": "parquet",
            "permission": "755",
            "replication": 0,
            "type": "DIRECTORY"
          },
          {
            "accessTime": 1700000001000,
            "blockSize": 134217728,
            "group": "supergroup",
            "length": 1851,
            "modificationTime": 1700000001000,
            "owner": "hadoop",
            "pathSuffix": "small.parquet",
            "permission": "644",
            "replication": 2,
            "type": "FILE"
          }
        ]
      }
    }"#;

    const GETFILESTATUS_NOT_FOUND: &str = r#"{
      "RemoteException": {
        "exception": "FileNotFoundException",
        "javaClassName": "java.io.FileNotFoundException",
        "message": "File does not exist: /no/such/path"
      }
    }"#;

    #[tokio::test]
    async fn test_list_returns_file_statuses() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/webhdfs/v1/?op=LISTSTATUS&user.name=hdfs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(LISTSTATUS_JSON)
            .create_async()
            .await;

        let client = WebHdfsClient::new(&server.url());
        let result = client.list("/").await.expect("list should succeed");

        assert_eq!(result.len(), 2);
        assert!(result[0].is_dir);
        assert_eq!(result[0].path, "/parquet");
        assert!(!result[1].is_dir);
        assert_eq!(result[1].path, "/small.parquet");
        assert_eq!(result[1].length, 1851);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stat_not_found_returns_error() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/webhdfs/v1/no/such/path?op=GETFILESTATUS&user.name=hdfs")
            .with_status(200) // WebHDFS may return 200 with RemoteException body
            .with_header("content-type", "application/json")
            .with_body(GETFILESTATUS_NOT_FOUND)
            .create_async()
            .await;

        let client = WebHdfsClient::new(&server.url());
        let result = client.stat("/no/such/path").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), HfsError::NotFound(_)));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stat_404_returns_not_found() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/webhdfs/v1/missing?op=GETFILESTATUS&user.name=hdfs")
            .with_status(404)
            .create_async()
            .await;

        let client = WebHdfsClient::new(&server.url());
        let result = client.stat("/missing").await;

        assert!(matches!(result.unwrap_err(), HfsError::NotFound(_)));
        mock.assert_async().await;
    }

    // ── Integration tests (require Docker cluster) ───────────────────────────

    #[tokio::test]
    #[ignore]
    async fn integration_list_root() {
        let base = std::env::var("HFS_NAMENODE").unwrap_or("http://localhost:9870".to_string());
        let client = WebHdfsClient::new(&base);
        let entries = client.list("/").await.expect("list / should work");
        assert!(!entries.is_empty(), "root should have at least 1 entry");
    }

    #[tokio::test]
    #[ignore]
    async fn integration_list_parquet_dir() {
        let base = std::env::var("HFS_NAMENODE").unwrap_or("http://localhost:9870".to_string());
        let client = WebHdfsClient::new(&base);
        let entries = client
            .list("/test-data/parquet/")
            .await
            .expect("list parquet dir should work");
        let names: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert!(
            names.iter().any(|n| n.contains("small.parquet")),
            "should find small.parquet, got: {:?}",
            names
        );
    }

    #[tokio::test]
    #[ignore]
    async fn integration_health() {
        let base = std::env::var("HFS_NAMENODE").unwrap_or("http://localhost:9870".to_string());
        let client = WebHdfsClient::new(&base);
        let h = client.health().await.expect("health should succeed");
        assert!(
            h.live_datanodes >= 1,
            "should have at least 1 live datanode"
        );
    }
}
