// ParquetInspector — legge schema e statistiche dal footer Parquet
//
// COME FUNZIONA:
// Il formato Parquet mette il footer alla fine del file:
//   [ROW_GROUP_0][ROW_GROUP_1]...[FOOTER_BYTES][4-byte footer_len][b"PAR1"]
//
// Per leggere lo schema servono solo gli ultimi (footer_len + 8) byte.
// Su HDFS da 500MB, leggiamo ~50KB invece di 500MB.
//
// POTENZIALE CONTRIBUZIONE UPSTREAM hdfs-native:
// Se hdfs-native non supporta read_range con offset negativo (dal fondo),
// aprire issue su datafusion-contrib/datafusion-hdfs-native e PR con fix.

use crate::{SeekableReader, schema::{Schema, Field, FieldType, SchemaSource}};
use anyhow::{anyhow, Result};

pub struct ParquetInspector;

/// Statistiche di colonna dal footer Parquet
#[derive(Debug, serde::Serialize)]
pub struct ColumnStats {
    pub name: String,
    pub field_type: FieldType,
    pub null_count: Option<i64>,
    pub distinct_count: Option<i64>,
    pub min_value: Option<String>,  // serializzato come stringa per semplicità
    pub max_value: Option<String>,
}

/// Informazioni estratte dal footer, senza leggere dati
#[derive(Debug, serde::Serialize)]
pub struct ParquetMeta {
    pub schema: Schema,
    pub row_group_count: usize,
    pub row_count: u64,           // somma di tutte le row_group rows
    pub column_stats: Vec<ColumnStats>,
    pub created_by: Option<String>,  // es. "parquet-mr version 1.12.0"
    pub key_value_metadata: std::collections::HashMap<String, String>,
}

impl ParquetInspector {
    /// Legge solo il footer del file Parquet — O(footer_size), non O(file_size)
    pub async fn inspect(reader: &dyn SeekableReader, path: &str) -> Result<ParquetMeta> {
        let file_size = reader.file_size().await?;

        if file_size < 12 {
            return Err(anyhow!("File too small to be a valid Parquet file: {} bytes", file_size));
        }

        // Step 1: leggi gli ultimi 8 byte per trovare la lunghezza del footer e il magic
        let tail = reader.read_range(file_size - 8, 8).await?;

        // Verifica magic PAR1
        if &tail[4..8] != b"PAR1" {
            return Err(anyhow!("Not a Parquet file (magic bytes mismatch)"));
        }

        let footer_len = u32::from_le_bytes(tail[0..4].try_into()?) as u64;

        if footer_len == 0 || footer_len > file_size - 8 {
            return Err(anyhow!("Invalid Parquet footer length: {}", footer_len));
        }

        // Step 2: leggi solo il footer
        let footer_offset = file_size - 8 - footer_len;
        let footer_bytes = reader.read_range(footer_offset, footer_len).await?;

        // Step 3: deserializza il FileMetaData Thrift/Protobuf
        // Usiamo il crate parquet di Apache Arrow
        Self::parse_footer(&footer_bytes, path, file_size)
    }

    fn parse_footer(footer_bytes: &[u8], path: &str, _file_size: u64) -> Result<ParquetMeta> {
        use parquet::file::metadata::ParquetMetaData;
        use parquet::file::footer::decode_footer;

        // Il crate parquet di Arrow espone decode_footer per parsing da bytes
        // TODO: usare l'API pubblica corretta del crate parquet
        // Al momento dell'implementazione verificare:
        //   parquet::file::footer::decode_metadata(bytes) oppure
        //   ParquetMetaData::try_from_encoded_file_metadata_data(bytes)

        // Placeholder — l'implementazione reale usa il crate parquet
        let _ = (footer_bytes, path, decode_footer);

        Err(anyhow!("TODO: implementare parsing footer con crate parquet"))
    }

    /// Converte schema Arrow Parquet nel tipo unificato Schema di hfs
    fn convert_schema(_parquet_schema: &parquet::schema::types::SchemaDescriptor, path: &str, row_groups: usize, row_count: u64) -> Schema {
        // TODO: walkthrough dei campi e conversione a FieldType
        Schema {
            fields: vec![],
            source: SchemaSource::Parquet {
                path: path.to_string(),
                row_groups,
                row_count,
            },
        }
    }
}
