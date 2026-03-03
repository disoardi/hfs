// HdfsClient trait — interfaccia comune per RPC e WebHDFS backend
//
// NOTA IMPLEMENTAZIONE:
// Le struct RpcClient e WebHdfsClient implementano questo trait.
// HdfsClientBuilder::build() seleziona automaticamente il backend:
//   1. Prova RPC nativo (hdfs-native) — porta 8020
//   2. Se fallisce o porta bloccata → WebHdfsClient — porta 9870
//
// DOWNSTREAM NOTE (per upstream hdfs-native):
// Operazioni filesystem che mancano in hdfs-native vanno segnalate
// come issue su datafusion-contrib/datafusion-hdfs-native prima di
// implementarle come workaround in WebHdfsClient.

use crate::error::HfsError;
use serde::{Deserialize, Serialize};

/// Metadata di un file o directory su HDFS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub length: u64,
    pub is_dir: bool,
    pub replication: u16,
    pub block_size: u64,
    pub modification_time: u64,
    pub access_time: u64,
    pub owner: String,
    pub group: String,
    pub permission: String,
}

/// Sommario contenuto di una directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSummary {
    pub directory_count: u64,
    pub file_count: u64,
    pub length: u64,
    pub space_consumed: u64,
    pub quota: i64, // -1 = nessuna quota
    pub space_quota: i64,
}

/// Informazioni su un singolo blocco HDFS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub block_id: u64,
    pub length: u64,
    pub corrupt: bool,
    pub datanode_locations: Vec<String>, // host:port di ogni replica
}

/// Stato di salute del cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealth {
    pub live_datanodes: u32,
    pub dead_datanodes: u32,
    pub stale_datanodes: u32,
    pub under_replicated_blocks: u64,
    pub corrupt_blocks: u64,
    pub capacity_total_bytes: u64,
    pub capacity_used_bytes: u64,
    pub capacity_remaining_bytes: u64,
    pub namenode_ha_state: Option<String>, // "active" | "standby" | None se non HA
}

/// Trait principale — implementato da RpcClient e WebHdfsClient
#[async_trait::async_trait]
pub trait HdfsClient: Send + Sync {
    async fn list(&self, path: &str) -> Result<Vec<FileStatus>, HfsError>;
    async fn stat(&self, path: &str) -> Result<FileStatus, HfsError>;
    async fn content_summary(&self, path: &str) -> Result<ContentSummary, HfsError>;
    async fn blocks(&self, path: &str) -> Result<Vec<BlockInfo>, HfsError>;
    async fn health(&self) -> Result<ClusterHealth, HfsError>;

    // Operazioni di scrittura — Tier 2
    async fn mkdir(&self, path: &str, create_parent: bool) -> Result<(), HfsError>;
    async fn delete(&self, path: &str, recursive: bool) -> Result<(), HfsError>;

    // Legge bytes di un range — usato da hfs-schema per leggere footer Parquet
    // senza scaricare l'intero file
    async fn read_range(&self, path: &str, offset: u64, length: u64) -> Result<Vec<u8>, HfsError>;
    async fn file_size(&self, path: &str) -> Result<u64, HfsError>;

    /// Quale backend è attivo
    fn backend_name(&self) -> &'static str;
}
