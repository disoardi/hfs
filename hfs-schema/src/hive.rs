// HiveMetastoreClient — schema tabella via Thrift API
//
// Hive Metastore espone una API Thrift sulla porta 9083 (default).
// Non richiede JVM lato client — si parla direttamente il protocollo Thrift
// come fa PySpark, PyHive, e altri client non-Java.
//
// Usato da `hfs drift` per confrontare schema file vs tabella Hive.

use crate::schema::{Schema, SchemaSource};
use anyhow::Result;

pub struct HiveMetastoreClient {
    pub host: String,
    pub port: u16,
    pub database: String,
}

impl HiveMetastoreClient {
    pub fn new(host: &str, port: u16, database: &str) -> Self {
        Self {
            host: host.to_string(),
            port,
            database: database.to_string(),
        }
    }

    /// Connessione default: host dalla config HDFS, porta 9083
    pub fn from_config(host: &str, database: &str) -> Self {
        Self::new(host, 9083, database)
    }

    /// Legge lo schema di una tabella Hive via Thrift
    /// Endpoint: GetTable(db, table) → Table.sd.cols
    pub async fn get_table_schema(&self, table: &str) -> Result<Schema> {
        // TODO: implementare con crate thrift
        // Protocollo: TCP → ThriftBinaryProtocol → HMS GetTable RPC
        //
        // Alternativa più semplice: Hive Metastore HTTP API (se abilitata)
        // GET http://{host}:9083/api/v1/database/{db}/table/{table}
        // → disponibile da HMS 3.0+
        //
        // Per cluster CDP/HDP con HMS 2.x: solo Thrift TCP
        let _ = table;
        Ok(Schema {
            fields: vec![],
            source: SchemaSource::Hive {
                database: self.database.clone(),
                table: table.to_string(),
            },
        })
    }

    /// Lista le tabelle in un database
    pub async fn list_tables(&self) -> Result<Vec<String>> {
        // TODO: GetAllTables(db) via Thrift
        Ok(vec![])
    }
}
