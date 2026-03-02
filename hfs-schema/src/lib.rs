// hfs-schema — Schema inspection per Parquet, Avro, ORC
//
// PRINCIPIO CHIAVE: legge solo footer/header del file, mai i dati.
// Per Parquet: Range request degli ultimi ~8KB (footer length + magic)
// Per Avro:    Legge solo i primi ~4KB (header con schema JSON)
// Per ORC:     Legge PostScript + Footer dagli ultimi byte
//
// Il SeekableReader trait è implementato da:
//   - HdfsRangeReader (via hfs-core RpcClient o WebHdfsClient)
//   - LocalFileReader (per test su file locali)
//   - futuro: S3RangeReader, GcsRangeReader via object_store

pub mod parquet;
pub mod avro;
pub mod schema;
pub mod hive;

#[cfg(feature = "orc")]
pub mod orc;

pub use schema::{Schema, Field, FieldType, SchemaDiff, DiffResult};
pub use parquet::ParquetInspector;
pub use avro::AvroInspector;

#[cfg(feature = "hive")]
pub use hive::HiveMetastoreClient;

/// Trait per lettura di range di byte — astratto su HDFS, locale, S3...
#[async_trait::async_trait]
pub trait SeekableReader: Send + Sync {
    async fn file_size(&self) -> anyhow::Result<u64>;
    async fn read_range(&self, offset: u64, length: u64) -> anyhow::Result<Vec<u8>>;
}

/// Rileva il formato di un file dall'estensione e/o magic bytes
pub fn detect_format(path: &str, first_bytes: &[u8]) -> FileFormat {
    if path.ends_with(".parquet") || first_bytes.starts_with(b"PAR1") {
        return FileFormat::Parquet;
    }
    if path.ends_with(".avro") || first_bytes.starts_with(b"Obj\x01") {
        return FileFormat::Avro;
    }
    if path.ends_with(".orc") || first_bytes.starts_with(b"ORC") {
        return FileFormat::Orc;
    }
    FileFormat::Unknown
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileFormat {
    Parquet,
    Avro,
    Orc,
    Unknown,
}
