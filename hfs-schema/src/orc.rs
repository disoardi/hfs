// ORC file inspector — reads PostScript + Footer from the last bytes only.
// This module requires the "orc" feature (dep:orc-rust).

/// ORC inspector stub — full implementation planned for Day 4.
pub struct OrcInspector;

impl OrcInspector {
    /// Inspect an ORC file and return its schema.
    /// ORC footer is in the last few bytes: PostScript + Footer + metadata length.
    pub async fn inspect(
        _reader: &dyn crate::SeekableReader,
        _path: &str,
    ) -> anyhow::Result<crate::schema::Schema> {
        anyhow::bail!("ORC inspection not yet implemented")
    }
}
