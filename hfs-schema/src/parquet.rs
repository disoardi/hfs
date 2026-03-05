// ParquetInspector — reads schema and statistics from the Parquet footer
//
// HOW IT WORKS:
// Parquet places the footer at the end of the file:
//   [ROW_GROUP_0][ROW_GROUP_1]...[FOOTER_BYTES][4-byte footer_len][b"PAR1"]
//
// Reading the schema requires only the last (footer_len + 8) bytes.
// On a 500 MB HDFS file this means ~50 KB instead of 500 MB — exactly 2 range
// requests: one for the 8-byte tail, one for the footer itself.
//
// UPSTREAM CANDIDATE (datafusion-hdfs-native):
// If hdfs-native ever adds a seek-from-end API, that would let us merge the two
// requests into one for very large footers.

use crate::{
    schema::{Field, FieldType, Schema, SchemaSource},
    SeekableReader,
};
use anyhow::{anyhow, Result};
// parquet::basic::Type is the physical type enum (confusingly named, different from
// parquet::schema::types::Type which represents a schema node).
use parquet::basic::{ConvertedType, LogicalType, Repetition, Type as PhysicalType};
use parquet::file::statistics::Statistics;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ParquetInspector;

/// Column statistics aggregated from the Parquet footer (no data read).
#[derive(Debug, serde::Serialize)]
pub struct ColumnStats {
    pub name: String,
    pub field_type: FieldType,
    pub null_count: Option<i64>,
    pub distinct_count: Option<i64>,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
}

/// All metadata extracted from the footer, without reading any data blocks.
#[derive(Debug, serde::Serialize)]
pub struct ParquetMeta {
    pub schema: Schema,
    pub row_group_count: usize,
    pub row_count: u64,
    pub column_stats: Vec<ColumnStats>,
    pub created_by: Option<String>,
    pub key_value_metadata: HashMap<String, String>,
}

impl ParquetInspector {
    /// Read only the Parquet footer — O(footer_size), not O(file_size).
    /// Makes exactly 2 range requests to the underlying storage.
    pub async fn inspect(reader: &dyn SeekableReader, path: &str) -> Result<ParquetMeta> {
        let file_size = reader.file_size().await?;

        if file_size < 12 {
            return Err(anyhow!(
                "File too small to be a valid Parquet file: {} bytes",
                file_size
            ));
        }

        // Request 1: last 8 bytes → footer_len (4 bytes LE) + magic "PAR1" (4 bytes)
        let tail = reader.read_range(file_size - 8, 8).await?;

        if &tail[4..8] != b"PAR1" {
            return Err(anyhow!(
                "Not a Parquet file: magic bytes mismatch (expected PAR1)"
            ));
        }

        let footer_len = u32::from_le_bytes(tail[0..4].try_into()?) as u64;

        if footer_len == 0 || footer_len > file_size - 8 {
            return Err(anyhow!("Invalid Parquet footer length: {}", footer_len));
        }

        // Request 2: the actual footer (Thrift-encoded FileMetaData)
        let footer_offset = file_size - 8 - footer_len;
        let footer_bytes = reader.read_range(footer_offset, footer_len).await?;

        parse_footer(&footer_bytes, path)
    }
}

// ─── Footer parsing ───────────────────────────────────────────────────────────

fn parse_footer(footer_bytes: &[u8], path: &str) -> Result<ParquetMeta> {
    use parquet::file::footer::decode_metadata;

    let metadata = decode_metadata(footer_bytes)
        .map_err(|e| anyhow!("Failed to decode Parquet footer: {}", e))?;

    let file_meta = metadata.file_metadata();
    let schema_descr = file_meta.schema_descr();

    // Map top-level schema fields to our common representation.
    let fields: Vec<Field> = schema_descr
        .root_schema()
        .get_fields()
        .iter()
        .map(parquet_type_to_field)
        .collect();

    // Row count: sum across all row groups.
    let row_count: u64 = metadata
        .row_groups()
        .iter()
        .map(|rg| rg.num_rows() as u64)
        .sum();

    let row_group_count = metadata.num_row_groups();

    // Key-value metadata (e.g. Arrow schema, Spark version, writer library).
    let key_value_metadata: HashMap<String, String> = file_meta
        .key_value_metadata()
        .map(|kvs| {
            kvs.iter()
                .filter_map(|kv| kv.value.as_ref().map(|v| (kv.key.clone(), v.clone())))
                .collect()
        })
        .unwrap_or_default();

    // Per-column stats aggregated across all row groups.
    let column_stats = build_column_stats(&metadata, schema_descr);

    let schema = Schema {
        fields,
        source: SchemaSource::Parquet {
            path: path.to_string(),
            row_groups: row_group_count,
            row_count,
        },
    };

    Ok(ParquetMeta {
        schema,
        row_group_count,
        row_count,
        column_stats,
        created_by: file_meta.created_by().map(str::to_string),
        key_value_metadata,
    })
}

// ─── Type mapping ─────────────────────────────────────────────────────────────

fn parquet_type_to_field(t: &Arc<parquet::schema::types::Type>) -> Field {
    let info = t.get_basic_info();
    let name = info.name().to_string();
    // REQUIRED fields are non-nullable; OPTIONAL and REPEATED are nullable.
    let nullable = !info.has_repetition() || info.repetition() != Repetition::REQUIRED;

    let field_type = if t.is_primitive() {
        map_primitive_type(
            t.get_physical_type(),
            info.logical_type(),
            info.converted_type(),
        )
    } else {
        map_group_type(t.get_fields(), info.logical_type())
    };

    Field {
        name,
        field_type,
        nullable,
        metadata: HashMap::new(),
    }
}

fn map_primitive_type(
    physical: PhysicalType,
    logical: Option<LogicalType>,
    converted: ConvertedType,
) -> FieldType {
    match physical {
        PhysicalType::BOOLEAN => FieldType::Boolean,

        PhysicalType::INT32 => match &logical {
            Some(LogicalType::Integer { bit_width: 8, .. }) => FieldType::Int8,
            Some(LogicalType::Integer { bit_width: 16, .. }) => FieldType::Int16,
            Some(LogicalType::Date) => FieldType::Date32,
            Some(LogicalType::Decimal { precision, scale }) => FieldType::Decimal {
                precision: *precision as u8,
                scale: *scale as i8,
            },
            _ if converted == ConvertedType::DATE => FieldType::Date32,
            _ if converted == ConvertedType::DECIMAL => FieldType::Decimal {
                precision: 0,
                scale: 0,
            },
            _ => FieldType::Int32,
        },

        PhysicalType::INT64 => match &logical {
            Some(LogicalType::Timestamp {
                is_adjusted_to_u_t_c,
                ..
            }) => FieldType::Timestamp {
                timezone: if *is_adjusted_to_u_t_c {
                    Some("UTC".to_string())
                } else {
                    None
                },
            },
            Some(LogicalType::Decimal { precision, scale }) => FieldType::Decimal {
                precision: *precision as u8,
                scale: *scale as i8,
            },
            _ if converted == ConvertedType::TIMESTAMP_MILLIS
                || converted == ConvertedType::TIMESTAMP_MICROS =>
            {
                FieldType::Timestamp { timezone: None }
            }
            _ => FieldType::Int64,
        },

        // INT96 is a legacy Impala/Hive timestamp (96-bit = 64-bit days + 32-bit nanos).
        PhysicalType::INT96 => FieldType::Timestamp { timezone: None },

        PhysicalType::FLOAT => FieldType::Float32,
        PhysicalType::DOUBLE => FieldType::Float64,

        PhysicalType::BYTE_ARRAY => match &logical {
            Some(LogicalType::String) | Some(LogicalType::Json) => FieldType::Utf8,
            Some(LogicalType::Decimal { precision, scale }) => FieldType::Decimal {
                precision: *precision as u8,
                scale: *scale as i8,
            },
            _ if converted == ConvertedType::UTF8
                || converted == ConvertedType::ENUM
                || converted == ConvertedType::JSON =>
            {
                FieldType::Utf8
            }
            _ => FieldType::Binary,
        },

        PhysicalType::FIXED_LEN_BYTE_ARRAY => match &logical {
            Some(LogicalType::Decimal { precision, scale }) => FieldType::Decimal {
                precision: *precision as u8,
                scale: *scale as i8,
            },
            Some(LogicalType::Uuid) => FieldType::Utf8,
            _ if converted == ConvertedType::DECIMAL => FieldType::Decimal {
                precision: 0,
                scale: 0,
            },
            _ => FieldType::Binary,
        },
    }
}

fn map_group_type(
    fields: &[Arc<parquet::schema::types::Type>],
    logical: Option<LogicalType>,
) -> FieldType {
    match &logical {
        Some(LogicalType::List) => {
            // Standard 3-level list encoding:
            //   optional group field_id=N (List) {
            //     repeated group list { optional <element_type> element; }
            //   }
            let element_type = fields
                .first()
                .map(|list_group| {
                    if list_group.is_group() {
                        list_group
                            .get_fields()
                            .first()
                            .map(parquet_type_to_field)
                            .map(|f| f.field_type)
                            .unwrap_or(FieldType::Unknown("element".to_string()))
                    } else {
                        parquet_type_to_field(list_group).field_type
                    }
                })
                .unwrap_or(FieldType::Unknown("element".to_string()));

            FieldType::List(Box::new(element_type))
        }

        Some(LogicalType::Map) => {
            // Standard map encoding:
            //   required group field_id=N (Map) {
            //     repeated group key_value { required <key>; optional <value>; }
            //   }
            let (key_type, val_type) = fields
                .first()
                .filter(|f| f.is_group())
                .map(|kv_group| {
                    let kv = kv_group.get_fields();
                    let key = kv
                        .first()
                        .map(|f| parquet_type_to_field(f).field_type)
                        .unwrap_or(FieldType::Unknown("key".to_string()));
                    let val = kv
                        .get(1)
                        .map(|f| parquet_type_to_field(f).field_type)
                        .unwrap_or(FieldType::Unknown("value".to_string()));
                    (key, val)
                })
                .unwrap_or((
                    FieldType::Unknown("key".to_string()),
                    FieldType::Unknown("value".to_string()),
                ));

            FieldType::Map {
                key: Box::new(key_type),
                value: Box::new(val_type),
            }
        }

        _ => {
            // Nested struct.
            let nested = fields.iter().map(parquet_type_to_field).collect();
            FieldType::Struct(nested)
        }
    }
}

// ─── Column statistics ────────────────────────────────────────────────────────

fn build_column_stats(
    metadata: &parquet::file::metadata::ParquetMetaData,
    schema_descr: &parquet::schema::types::SchemaDescriptor,
) -> Vec<ColumnStats> {
    let num_columns = schema_descr.num_columns();

    // Initialise one entry per leaf column.
    let mut stats: Vec<ColumnStats> = (0..num_columns)
        .map(|i| {
            let col = schema_descr.column(i);
            ColumnStats {
                name: col.path().string(),
                field_type: map_primitive_type(
                    col.physical_type(),
                    col.logical_type(),
                    col.converted_type(),
                ),
                null_count: Some(0),
                distinct_count: None,
                min_value: None,
                max_value: None,
            }
        })
        .collect();

    // Aggregate across row groups.
    for rg in metadata.row_groups() {
        for (col_idx, col_chunk) in rg.columns().iter().enumerate() {
            if col_idx >= stats.len() {
                break;
            }
            let Some(stat) = col_chunk.statistics() else {
                continue;
            };

            // Sum null counts across row groups.
            stats[col_idx].null_count =
                Some(stats[col_idx].null_count.unwrap_or(0) + stat.null_count() as i64);

            // Distinct count: take the maximum (conservative estimate).
            if let Some(dc) = stat.distinct_count() {
                stats[col_idx].distinct_count =
                    Some(stats[col_idx].distinct_count.unwrap_or(0).max(dc as i64));
            }

            // Min/max: first row group with values wins for brevity.
            if stats[col_idx].min_value.is_none() && stat.has_min_max_set() {
                stats[col_idx].min_value = format_stat_bytes(stat.min_bytes(), stat);
                stats[col_idx].max_value = format_stat_bytes(stat.max_bytes(), stat);
            }
        }
    }

    stats
}

/// Interpret a raw byte slice from Statistics as a human-readable string,
/// using the physical type of the statistic for correct byte interpretation.
fn format_stat_bytes(bytes: &[u8], stat: &Statistics) -> Option<String> {
    match stat.physical_type() {
        PhysicalType::BOOLEAN => Some(
            if bytes.first().copied().unwrap_or(0) != 0 {
                "true"
            } else {
                "false"
            }
            .to_string(),
        ),
        PhysicalType::INT32 => {
            let arr: [u8; 4] = bytes.try_into().ok()?;
            Some(i32::from_le_bytes(arr).to_string())
        }
        PhysicalType::INT64 => {
            let arr: [u8; 8] = bytes.try_into().ok()?;
            Some(i64::from_le_bytes(arr).to_string())
        }
        PhysicalType::FLOAT => {
            let arr: [u8; 4] = bytes.try_into().ok()?;
            Some(format!("{:.6}", f32::from_le_bytes(arr)))
        }
        PhysicalType::DOUBLE => {
            let arr: [u8; 8] = bytes.try_into().ok()?;
            Some(format!("{:.6}", f64::from_le_bytes(arr)))
        }
        PhysicalType::BYTE_ARRAY => match std::str::from_utf8(bytes) {
            Ok(s) => Some(s.to_string()),
            Err(_) => Some(format!("0x{}", hex_encode(bytes))),
        },
        _ => None,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SeekableReader;

    /// In-memory SeekableReader backed by a byte buffer — used for unit tests.
    struct MemReader {
        data: Vec<u8>,
    }

    #[async_trait::async_trait]
    impl SeekableReader for MemReader {
        async fn file_size(&self) -> anyhow::Result<u64> {
            Ok(self.data.len() as u64)
        }

        async fn read_range(&self, offset: u64, length: u64) -> anyhow::Result<Vec<u8>> {
            let start = offset as usize;
            let end = (offset + length) as usize;
            Ok(self.data[start..end.min(self.data.len())].to_vec())
        }
    }

    #[tokio::test]
    async fn test_inspect_rejects_too_small_file() {
        let reader = MemReader { data: vec![0u8; 4] };
        let result = ParquetInspector::inspect(&reader, "/test.parquet").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too small"));
    }

    #[tokio::test]
    async fn test_inspect_rejects_wrong_magic() {
        // 12 bytes, valid size but wrong magic.
        let mut data = vec![0u8; 12];
        // footer_len = 4 (little-endian), then "NOPE" as magic
        data[4..8].copy_from_slice(&4u32.to_le_bytes());
        data[8..12].copy_from_slice(b"NOPE");
        let reader = MemReader { data };
        let result = ParquetInspector::inspect(&reader, "/test.parquet").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("magic"));
    }

    #[test]
    fn test_map_primitive_boolean() {
        let ft = map_primitive_type(PhysicalType::BOOLEAN, None, ConvertedType::NONE);
        assert!(matches!(ft, FieldType::Boolean));
    }

    #[test]
    fn test_map_primitive_int32_date_logical() {
        let ft = map_primitive_type(
            PhysicalType::INT32,
            Some(LogicalType::Date),
            ConvertedType::NONE,
        );
        assert!(matches!(ft, FieldType::Date32));
    }

    #[test]
    fn test_map_primitive_int32_date_converted() {
        let ft = map_primitive_type(PhysicalType::INT32, None, ConvertedType::DATE);
        assert!(matches!(ft, FieldType::Date32));
    }

    #[test]
    fn test_map_primitive_byte_array_utf8_logical() {
        let ft = map_primitive_type(
            PhysicalType::BYTE_ARRAY,
            Some(LogicalType::String),
            ConvertedType::NONE,
        );
        assert!(matches!(ft, FieldType::Utf8));
    }

    #[test]
    fn test_map_primitive_byte_array_utf8_converted() {
        let ft = map_primitive_type(PhysicalType::BYTE_ARRAY, None, ConvertedType::UTF8);
        assert!(matches!(ft, FieldType::Utf8));
    }

    #[test]
    fn test_map_primitive_byte_array_binary() {
        let ft = map_primitive_type(PhysicalType::BYTE_ARRAY, None, ConvertedType::NONE);
        assert!(matches!(ft, FieldType::Binary));
    }

    #[test]
    fn test_map_primitive_int32_int8() {
        let ft = map_primitive_type(
            PhysicalType::INT32,
            Some(LogicalType::Integer {
                bit_width: 8,
                is_signed: true,
            }),
            ConvertedType::NONE,
        );
        assert!(matches!(ft, FieldType::Int8));
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex_encode(&[0xDE, 0xAD, 0xBE, 0xEF]), "deadbeef");
    }

    /// Integration test: inspect a real Parquet fixture.
    /// Run with: cargo test -p hfs-schema -- --include-ignored
    #[tokio::test]
    #[ignore]
    async fn integration_inspect_local_parquet() {
        let path = "tests/fixtures/small.parquet";
        let data = std::fs::read(path).expect("fixture not found");
        let reader = MemReader { data };
        let meta = ParquetInspector::inspect(&reader, path)
            .await
            .expect("inspect failed");
        assert!(meta.row_count > 0, "expected at least one row");
        assert!(
            !meta.schema.fields.is_empty(),
            "expected at least one field"
        );
        println!(
            "Schema: {} fields, {} rows, {} row groups",
            meta.schema.fields.len(),
            meta.row_count,
            meta.row_group_count
        );
    }
}
