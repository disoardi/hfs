// HdfsRangeReader — bridges HdfsClient to the SeekableReader trait.
//
// This adapter wraps an HdfsClient + a file path so that schema inspectors
// (ParquetInspector, AvroInspector) can perform range reads against HDFS
// without depending on a specific client implementation.
//
// The two underlying calls map as:
//   SeekableReader::file_size()       → HdfsClient::file_size(path)
//   SeekableReader::read_range(o, l)  → HdfsClient::read_range(path, o, l)

use crate::SeekableReader;
use anyhow::Result;
use hfs_core::HdfsClient;

/// Adapts a borrowed HdfsClient for a specific path to the SeekableReader trait.
pub struct HdfsRangeReader<'a> {
    client: &'a dyn HdfsClient,
    path: String,
}

impl<'a> HdfsRangeReader<'a> {
    pub fn new(client: &'a dyn HdfsClient, path: &str) -> Self {
        Self {
            client,
            path: path.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl SeekableReader for HdfsRangeReader<'_> {
    async fn file_size(&self) -> Result<u64> {
        self.client
            .file_size(&self.path)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    async fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>> {
        self.client
            .read_range(&self.path, offset, length)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }
}
