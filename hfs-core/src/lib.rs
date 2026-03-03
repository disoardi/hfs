// hfs-core — HDFS client library
// Dual-mode: RPC nativo (hdfs-native) con fallback WebHDFS REST
//
// ARCHITETTURA:
//   HdfsClient trait  →  implementato da RpcClient e WebHdfsClient
//   HdfsClientBuilder →  auto-seleziona backend, legge config

pub mod client;
pub mod config;
pub mod error;
pub mod webhdfs;

pub use client::{BlockInfo, ClusterHealth, ContentSummary, FileStatus, HdfsClient};
pub use config::HdfsConfig;
pub use error::HfsError;
pub use webhdfs::WebHdfsClient;
