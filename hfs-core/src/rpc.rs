// RpcClient — native HDFS RPC backend via hdfs-native
//
// Uses the protobuf-over-TCP HDFS protocol (port 8020, default).
// No JVM required — hdfs-native is a pure-Rust implementation of the
// Hadoop RPC wire protocol.
//
// LIMITATIONS (hdfs-native v0.9):
//   - FileStatus does not expose replication or block_size (set to 0)
//   - content_summary, blocks, health are not available via this backend;
//     those methods return HfsError::NotSupported.
//   - Use HdfsClientBuilder with auto mode for transparent fallback.
//
// UPSTREAM CANDIDATES for datafusion-hdfs-native:
//   - Add replication + block_size to FileStatus struct
//   - Expose get_content_summary() as a Client method

use crate::client::{BlockInfo, ClusterHealth, ContentSummary, FileStatus, HdfsClient};
use crate::error::{map_native_error, HfsError};
use async_trait::async_trait;
use hdfs_native::Client;

pub struct RpcClient {
    inner: Client,
}

impl RpcClient {
    /// Build an RPC client. Connection is lazy — actual TCP handshake happens
    /// on the first operation.
    pub fn new(namenode_uri: &str) -> Result<Self, HfsError> {
        let inner = Client::new(namenode_uri)
            .map_err(|e| HfsError::Connection(format!("RPC client init failed: {}", e)))?;
        Ok(Self { inner })
    }
}

/// Convert an hdfs-native FileStatus to our common FileStatus.
/// hdfs-native v0.9 does not include replication or block_size in FileStatus,
/// so those fields are set to 0.
fn map_file_status(fs: hdfs_native::client::FileStatus) -> FileStatus {
    FileStatus {
        path: fs.path,
        length: fs.length as u64,
        is_dir: fs.isdir,
        replication: 0, // not exposed by hdfs-native v0.9
        block_size: 0,  // not exposed by hdfs-native v0.9
        modification_time: fs.modification_time,
        access_time: fs.access_time,
        owner: fs.owner,
        group: fs.group,
        permission: format!("{:o}", fs.permission),
    }
}

#[async_trait]
impl HdfsClient for RpcClient {
    async fn list(&self, path: &str) -> Result<Vec<FileStatus>, HfsError> {
        let entries = self
            .inner
            .list_status(path, false)
            .await
            .map_err(|e| map_native_error(e, path))?;
        Ok(entries.into_iter().map(map_file_status).collect())
    }

    async fn stat(&self, path: &str) -> Result<FileStatus, HfsError> {
        let fs = self
            .inner
            .get_file_info(path)
            .await
            .map_err(|e| map_native_error(e, path))?;
        Ok(map_file_status(fs))
    }

    async fn content_summary(&self, _path: &str) -> Result<ContentSummary, HfsError> {
        Err(HfsError::NotSupported(
            "du/content_summary requires WebHDFS — use --backend webhdfs or auto".to_string(),
        ))
    }

    async fn blocks(&self, _path: &str) -> Result<Vec<BlockInfo>, HfsError> {
        Err(HfsError::NotSupported(
            "block locations require WebHDFS — use --backend webhdfs or auto".to_string(),
        ))
    }

    async fn health(&self) -> Result<ClusterHealth, HfsError> {
        Err(HfsError::NotSupported(
            "cluster health requires WebHDFS JMX — use --backend webhdfs or auto".to_string(),
        ))
    }

    async fn mkdir(&self, path: &str, create_parent: bool) -> Result<(), HfsError> {
        self.inner
            .mkdirs(path, 0o755, create_parent)
            .await
            .map_err(|e| map_native_error(e, path))?;
        Ok(())
    }

    async fn delete(&self, path: &str, recursive: bool) -> Result<(), HfsError> {
        self.inner
            .delete(path, recursive)
            .await
            .map_err(|e| map_native_error(e, path))?;
        Ok(())
    }

    async fn read_range(&self, path: &str, offset: u64, length: u64) -> Result<Vec<u8>, HfsError> {
        let reader = self
            .inner
            .read(path)
            .await
            .map_err(|e| map_native_error(e, path))?;
        let bytes = reader
            .read_range(offset as usize, length as usize)
            .await
            .map_err(|e| map_native_error(e, path))?;
        Ok(bytes.to_vec())
    }

    async fn file_size(&self, path: &str) -> Result<u64, HfsError> {
        Ok(self.stat(path).await?.length)
    }

    fn backend_name(&self) -> &'static str {
        "rpc"
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_client_new_invalid_url_returns_error() {
        // A URL without a host must fail at construction time.
        let result = RpcClient::new("hdfs://");
        assert!(result.is_err(), "expected Err for host-less URL, got Ok");
    }

    #[test]
    fn test_rpc_client_new_valid_url_succeeds() {
        // Construction is lazy — no network access here.
        let result = RpcClient::new("hdfs://127.0.0.1:8020");
        assert!(result.is_ok(), "expected Ok for valid URL");
    }

    /// Integration test — requires a live HDFS cluster via Docker.
    /// Run with: HFS_NAMENODE=hdfs://localhost:8020 cargo test -- --include-ignored
    #[tokio::test]
    #[ignore]
    async fn integration_rpc_list_root() {
        let namenode = std::env::var("HFS_NAMENODE").unwrap_or("hdfs://localhost:8020".to_string());
        if !namenode.starts_with("hdfs://") {
            return; // skip if pointing at WebHDFS
        }
        let client = RpcClient::new(&namenode).expect("client init");
        let entries = client.list("/").await.expect("list /");
        assert!(!entries.is_empty(), "/ should have at least one entry");
    }
}
