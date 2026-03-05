// HdfsClientBuilder — auto-selects backend based on config and availability
//
// Selection logic for --backend auto (RPC is preferred over WebHDFS):
//   1. If config has an hdfs:// URI (explicit or derived from bare host:port with RPC port)
//      → probe RPC within PROBE_TIMEOUT_MS; on success use RPC, on failure fall back to WebHDFS
//   2. Otherwise (explicit http:// URL) → WebHDFS directly
//
// Bare host:port heuristic (applied before builder, in config parsing):
//   port 9870 / 50070  → webhdfs_url  (WebHDFS path)
//   port 8020 / 8021 or unknown → namenode_uri as hdfs://  (RPC probe path)
//
// For --backend rpc:
//   - Connect via RPC; if construction fails, return error (no fallback)
// For --backend webhdfs:
//   - Always use WebHDFS, regardless of namenode_uri

use std::panic::AssertUnwindSafe;
use std::time::Duration;

use crate::client::HdfsClient;
use crate::config::HdfsConfig;
use crate::error::HfsError;
use crate::rpc::RpcClient;
use crate::webhdfs::WebHdfsClient;

/// Timeout for the auto-detection probe (ms).
/// Keep short so startup latency stays low when RPC is unreachable.
const PROBE_TIMEOUT_MS: u64 = 3_000;

pub struct HdfsClientBuilder;

impl HdfsClientBuilder {
    /// Build a client according to `config.preferred_backend`.
    /// Returns a trait object so callers are agnostic to the concrete type.
    pub async fn build(config: &HdfsConfig) -> Box<dyn HdfsClient> {
        match config.preferred_backend.as_str() {
            "rpc" => Self::build_rpc(config),
            "webhdfs" => Self::build_webhdfs(config),
            _ => Self::build_auto(config).await,
        }
    }

    fn build_webhdfs(config: &HdfsConfig) -> Box<dyn HdfsClient> {
        Box::new(WebHdfsClient::new(&config.effective_webhdfs_url()))
    }

    fn build_rpc(config: &HdfsConfig) -> Box<dyn HdfsClient> {
        let uri = config.namenode_uri.clone();
        // hdfs-native Client::new() can panic when OS user resolution fails (e.g. LDAP/AD
        // accounts without a local /etc/passwd entry). Catch that and fall back gracefully.
        match std::panic::catch_unwind(AssertUnwindSafe(|| RpcClient::new(&uri))) {
            Ok(Ok(c)) => Box::new(c),
            Ok(Err(e)) => {
                eprintln!("[hfs] RPC init failed ({}); falling back to WebHDFS", e);
                eprintln!("      Tip: set HADOOP_USER_NAME=<user> to fix user resolution");
                Box::new(WebHdfsClient::new(&config.effective_webhdfs_url()))
            }
            Err(_) => {
                eprintln!("[hfs] RPC init panicked (OS user resolution failed); falling back to WebHDFS");
                eprintln!("      Tip: set HADOOP_USER_NAME=<user> to fix this");
                Box::new(WebHdfsClient::new(&config.effective_webhdfs_url()))
            }
        }
    }

    async fn build_auto(config: &HdfsConfig) -> Box<dyn HdfsClient> {
        // Only attempt RPC when we have an explicit hdfs:// URI.
        if config.namenode_uri.starts_with("hdfs://") {
            let uri = config.namenode_uri.clone();
            // Wrap in catch_unwind: hdfs-native can panic on OS user resolution failure
            // (e.g. LDAP/AD accounts without a local /etc/passwd entry).
            let rpc_init =
                std::panic::catch_unwind(AssertUnwindSafe(|| RpcClient::new(&uri)));

            if let Ok(Ok(rpc)) = rpc_init {
                let probe =
                    tokio::time::timeout(Duration::from_millis(PROBE_TIMEOUT_MS), rpc.stat("/"))
                        .await;

                match probe {
                    Ok(Ok(_)) => {
                        return Box::new(rpc);
                    }
                    Ok(Err(HfsError::NotFound(_))) => {
                        // "/" might not exist but the cluster is reachable — use RPC.
                        return Box::new(rpc);
                    }
                    _ => {
                        // Timeout or connection error → fall through to WebHDFS.
                    }
                }
            }
            // RPC init panicked (user resolution) or construction failed → WebHDFS.
        }

        Box::new(WebHdfsClient::new(&config.effective_webhdfs_url()))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builder_forced_webhdfs() {
        let config = HdfsConfig {
            preferred_backend: "webhdfs".to_string(),
            ..Default::default()
        };
        let client = HdfsClientBuilder::build(&config).await;
        assert_eq!(client.backend_name(), "WebHDFS");
    }

    #[tokio::test]
    async fn test_builder_auto_no_hdfs_uri_uses_webhdfs() {
        // When namenode_uri is empty, auto must choose WebHDFS.
        let config = HdfsConfig {
            namenode_uri: String::new(),
            preferred_backend: "auto".to_string(),
            ..Default::default()
        };
        let client = HdfsClientBuilder::build(&config).await;
        assert_eq!(client.backend_name(), "WebHDFS");
    }

    #[tokio::test]
    async fn test_builder_auto_unreachable_rpc_falls_back_to_webhdfs() {
        // Port 19999 on loopback should be closed → connection refused → WebHDFS fallback.
        let config = HdfsConfig {
            namenode_uri: "hdfs://127.0.0.1:19999".to_string(),
            preferred_backend: "auto".to_string(),
            ..Default::default()
        };
        let client = HdfsClientBuilder::build(&config).await;
        // Even if RPC init succeeds (lazy), the probe will fail and fall back.
        assert_eq!(client.backend_name(), "WebHDFS");
    }

    /// Integration test — requires a live HDFS cluster.
    #[tokio::test]
    #[ignore]
    async fn integration_builder_auto_selects_rpc() {
        let namenode = std::env::var("HFS_NAMENODE").unwrap_or("hdfs://localhost:8020".to_string());
        if !namenode.starts_with("hdfs://") {
            return;
        }
        let config = HdfsConfig {
            namenode_uri: namenode,
            preferred_backend: "auto".to_string(),
            ..Default::default()
        };
        let client = HdfsClientBuilder::build(&config).await;
        assert_eq!(client.backend_name(), "rpc");
    }
}
