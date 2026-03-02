// HdfsConfig — configurazione con lettura automatica core-site.xml
//
// Priorità (dal più basso al più alto):
//   1. Default built-in
//   2. /etc/hadoop/conf/core-site.xml
//   3. $HADOOP_CONF_DIR/core-site.xml
//   4. ~/.hfs/config.toml
//   5. Flag CLI passati a HdfsConfig::builder()

use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct HdfsConfig {
    /// URI namenode es. "hdfs://namenode.corp.com:8020"
    pub namenode_uri: String,

    /// Utente HDFS per operazioni non kerberizzate
    pub hdfs_user: Option<String>,

    /// URL WebHDFS es. "http://namenode.corp.com:9870"
    /// Se None, viene derivato da namenode_uri
    pub webhdfs_url: Option<String>,

    /// Kerberos principal (es. "davide@CORP.COM")
    pub kerberos_principal: Option<String>,

    /// Path keytab per kinit automatico
    pub keytab_path: Option<PathBuf>,

    /// Timeout connessione in secondi
    pub connect_timeout_secs: u64,

    /// Backend preferito: "rpc" | "webhdfs" | "auto"
    pub preferred_backend: String,

    /// Tutte le proprietà lette da core-site.xml (per debug)
    pub raw_hadoop_props: HashMap<String, String>,
}

impl Default for HdfsConfig {
    fn default() -> Self {
        Self {
            namenode_uri: String::new(),
            hdfs_user: None,
            webhdfs_url: None,
            kerberos_principal: None,
            keytab_path: None,
            connect_timeout_secs: 30,
            preferred_backend: "auto".to_string(),
            raw_hadoop_props: HashMap::new(),
        }
    }
}

impl HdfsConfig {
    /// Costruisce configurazione leggendo tutte le sorgenti nell'ordine di priorità
    pub fn load() -> Result<Self> {
        let mut cfg = Self::default();

        // 1. Prova core-site.xml standard
        for path in Self::hadoop_conf_candidates() {
            if path.exists() {
                cfg.merge_from_core_site(&path)?;
                break;
            }
        }

        // 2. Override da ~/.hfs/config.toml se esiste
        // TODO: implementare parsing TOML

        // 3. Override da env vars
        cfg.merge_from_env();

        Ok(cfg)
    }

    /// Candidati path per core-site.xml nell'ordine di priorità
    fn hadoop_conf_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        if let Ok(dir) = std::env::var("HADOOP_CONF_DIR") {
            candidates.push(PathBuf::from(dir).join("core-site.xml"));
        }
        candidates.push(PathBuf::from("/etc/hadoop/conf/core-site.xml"));
        candidates.push(PathBuf::from("/etc/hadoop/conf/hdfs-site.xml"));

        candidates
    }

    /// Legge le proprietà rilevanti da core-site.xml
    /// Usa quick-xml per parsing minimale — non è un lettore XML completo
    fn merge_from_core_site(&mut self, path: &PathBuf) -> Result<()> {
        // TODO: implementare con quick-xml
        // Campi da leggere:
        //   fs.defaultFS                      → namenode_uri
        //   dfs.namenode.rpc-address          → host:porta RPC
        //   hadoop.security.authentication    → "kerberos" | "simple"
        //   dfs.client.use.datanode.hostname  → bool
        let _ = path; // placeholder
        Ok(())
    }

    fn merge_from_env(&mut self) {
        if let Ok(nn) = std::env::var("HFS_NAMENODE") {
            self.namenode_uri = nn;
        }
        if let Ok(user) = std::env::var("HFS_USER").or_else(|_| std::env::var("HADOOP_USER_NAME")) {
            self.hdfs_user = Some(user);
        }
        if let Ok(backend) = std::env::var("HFS_BACKEND") {
            self.preferred_backend = backend;
        }
    }

    /// Deriva URL WebHDFS da namenode_uri se non esplicitamente configurato
    pub fn webhdfs_url(&self) -> String {
        if let Some(ref url) = self.webhdfs_url {
            return url.clone();
        }
        // hdfs://host:8020 → http://host:9870
        self.namenode_uri
            .replace("hdfs://", "http://")
            .replace(":8020", ":9870")
            .replace(":9000", ":9870")
    }
}
