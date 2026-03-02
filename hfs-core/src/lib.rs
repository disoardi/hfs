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

pub use client::{HdfsClient, FileStatus, ContentSummary, BlockInfo};
pub use config::HdfsConfig;
pub use error::HfsError;
