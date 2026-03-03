// Schema — rappresentazione unificata per Parquet, Avro, ORC e Hive
//
// Permette di confrontare schemi tra formati diversi senza dipendere
// da rappresentazioni interne dei singoli crate.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Schema {
    pub fields: Vec<Field>,
    pub source: SchemaSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub nullable: bool,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FieldType {
    Boolean,
    Int8,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    Decimal {
        precision: u8,
        scale: i8,
    },
    Utf8,
    LargeUtf8,
    Binary,
    LargeBinary,
    Date32,
    Date64,
    Timestamp {
        timezone: Option<String>,
    },
    List(Box<FieldType>),
    Map {
        key: Box<FieldType>,
        value: Box<FieldType>,
    },
    Struct(Vec<Field>),
    Unknown(String), // tipo non mappato — preserva il nome originale
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SchemaSource {
    Parquet {
        path: String,
        row_groups: usize,
        row_count: u64,
    },
    Avro {
        path: String,
    },
    Orc {
        path: String,
    },
    Hive {
        database: String,
        table: String,
    },
}

/// Risultato del confronto tra due schemi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDiff {
    pub source_a: SchemaSource,
    pub source_b: SchemaSource,
    pub changes: Vec<DiffResult>,
    pub compatible: bool, // false = schema change breaking
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffResult {
    Added {
        field: Field,
    }, // campo in B non presente in A
    Removed {
        field: Field,
    }, // campo in A non presente in B (breaking)
    Changed {
        name: String,
        from: FieldType,
        to: FieldType,
        breaking: bool,
    },
    Reordered {
        name: String,
        from_pos: usize,
        to_pos: usize,
    },
}

impl SchemaDiff {
    pub fn compare(a: &Schema, b: &Schema) -> Self {
        let mut changes = Vec::new();

        // Campi rimossi (breaking)
        for fa in &a.fields {
            if !b.fields.iter().any(|fb| fb.name == fa.name) {
                changes.push(DiffResult::Removed { field: fa.clone() });
            }
        }

        // Campi aggiunti e modificati
        for fb in &b.fields {
            match a.fields.iter().find(|fa| fa.name == fb.name) {
                None => changes.push(DiffResult::Added { field: fb.clone() }),
                Some(fa) if fa.field_type != fb.field_type => {
                    let breaking = !Self::is_compatible_change(&fa.field_type, &fb.field_type);
                    changes.push(DiffResult::Changed {
                        name: fb.name.clone(),
                        from: fa.field_type.clone(),
                        to: fb.field_type.clone(),
                        breaking,
                    });
                }
                _ => {}
            }
        }

        let compatible = !changes.iter().any(|c| {
            matches!(
                c,
                DiffResult::Removed { .. } | DiffResult::Changed { breaking: true, .. }
            )
        });

        Self {
            source_a: a.source.clone(),
            source_b: b.source.clone(),
            changes,
            compatible,
        }
    }

    /// Widening compatibile: Int32→Int64, Float32→Float64, ecc.
    fn is_compatible_change(from: &FieldType, to: &FieldType) -> bool {
        matches!(
            (from, to),
            (FieldType::Int32, FieldType::Int64)
                | (FieldType::Float32, FieldType::Float64)
                | (FieldType::Utf8, FieldType::LargeUtf8)
                | (FieldType::Binary, FieldType::LargeBinary)
        )
    }
}
