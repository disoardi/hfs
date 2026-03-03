// hfs-core — HDFS client library
// Dual-mode: RPC nativo (hdfs-native) con fallback WebHDFS REST
//
// ARCHITETTURA:
//   HdfsClient trait  →  implementato da RpcClient e WebHdfsClient
//   HdfsClientBuilder →  auto-seleziona backend, legge config

pub mod builder;
pub mod client;
pub mod config;
pub mod error;
pub mod rpc;
pub mod webhdfs;

pub use builder::HdfsClientBuilder;
pub use client::{BlockInfo, ClusterHealth, ContentSummary, FileStatus, HdfsClient};
pub use config::HdfsConfig;
pub use error::HfsError;
pub use rpc::RpcClient;
pub use webhdfs::WebHdfsClient;
