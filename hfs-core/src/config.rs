// HdfsConfig — configuration with automatic core-site.xml reading
//
// Priority (lowest to highest):
//   1. Built-in defaults
//   2. /etc/hadoop/conf/  (core-site.xml, hdfs-site.xml)
//   3. ~/.hfs/profile.env  (or --env-file <path>)
//      └─ may reference HADOOP_CONF_DIR → loads that dir's XML files
//   4. Environment variables (HFS_NAMENODE, HFS_USER, HFS_BACKEND, …)
//   5. CLI flags (--namenode, --backend, …)  — highest priority

use crate::error::HfsError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct HdfsConfig {
    /// NameNode URI, e.g. "hdfs://namenode.corp.com:8020"
    pub namenode_uri: String,

    /// HDFS user for non-Kerberos operations
    pub hdfs_user: Option<String>,

    /// Explicit WebHDFS URL override, e.g. "http://namenode.corp.com:9870"
    /// If None, derived from namenode_uri
    pub webhdfs_url: Option<String>,

    /// Kerberos principal (e.g. "user@CORP.COM")
    pub kerberos_principal: Option<String>,

    /// Keytab path for automatic kinit
    pub keytab_path: Option<PathBuf>,

    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,

    /// Preferred backend: "rpc" | "webhdfs" | "auto"
    pub preferred_backend: String,

    /// All properties read from core-site.xml (for debug / raw access)
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
    /// Build configuration by reading all sources in priority order.
    ///
    /// `env_file`: path to a .env profile (from `--env-file`).
    ///   If `None`, falls back to `~/.hfs/profile.env`.
    ///   The profile may contain `HADOOP_CONF_DIR=/path/to/conf` which causes
    ///   hfs to read core-site.xml, hdfs-site.xml (and other Hadoop XML files)
    ///   from that directory.
    pub fn load(env_file: Option<&Path>) -> Result<Self, HfsError> {
        let mut cfg = Self::default();

        // Layer 1 — system Hadoop conf (lowest priority)
        for dir in ["/etc/hadoop/conf", "/usr/lib/hadoop/etc/hadoop"] {
            let p = PathBuf::from(dir);
            if p.exists() {
                cfg.load_hadoop_conf_dir(&p);
                break;
            }
        }

        // Layer 2 — per-user profile or explicit --env-file
        match env_file {
            Some(path) => cfg.merge_from_env_file(path),
            None => cfg.merge_from_profile_env(),
        }

        // Layer 3 — process environment variables (highest priority)
        cfg.merge_from_env();

        if cfg.namenode_uri.is_empty() && cfg.webhdfs_url.is_none() {
            cfg.namenode_uri = "hdfs://localhost:8020".to_string();
        }

        Ok(cfg)
    }

    /// The effective HDFS user for this session.
    /// Defaults to "hdfs" when no user is explicitly configured, so that
    /// connections from non-Hadoop Linux users (e.g. LDAP/AD accounts without
    /// a local /etc/passwd entry) work out of the box.
    pub fn effective_user(&self) -> &str {
        self.hdfs_user.as_deref().unwrap_or("hdfs")
    }

    /// Load all Hadoop XML config files from a directory.
    ///
    /// Files read (in order, later values override earlier ones within the same key):
    ///   core-site.xml, hdfs-site.xml, mapred-site.xml, yarn-site.xml
    ///
    /// Properties extracted:
    ///   fs.defaultFS / fs.default.name     → namenode_uri
    ///   dfs.namenode.http-address          → webhdfs_url (if no http scheme)
    ///   dfs.namenode.rpc-address           → namenode_uri (hdfs://host:port)
    ///   hadoop.security.authentication     → kerberos indicator
    pub fn load_hadoop_conf_dir(&mut self, dir: &Path) {
        const HADOOP_XML_FILES: &[&str] =
            &["core-site.xml", "hdfs-site.xml", "mapred-site.xml", "yarn-site.xml"];

        for filename in HADOOP_XML_FILES {
            let path = dir.join(filename);
            if path.exists() {
                let _ = self.merge_from_core_site(&path);
            }
        }

        // After merging all files, extract hdfs-site properties not handled by merge_from_core_site
        self.apply_hdfs_site_props();
    }

    /// Apply dfs.* properties from raw_hadoop_props that aren't covered by merge_from_core_site.
    fn apply_hdfs_site_props(&mut self) {
        // dfs.namenode.http-address → webhdfs_url (only if not already set by fs.defaultFS)
        if self.webhdfs_url.is_none() {
            if let Some(addr) = self.raw_hadoop_props.get("dfs.namenode.http-address").cloned() {
                if !addr.is_empty() {
                    self.webhdfs_url = Some(if addr.starts_with("http") {
                        addr
                    } else {
                        format!("http://{}", addr)
                    });
                }
            }
        }
        // dfs.namenode.rpc-address → namenode_uri (only if fs.defaultFS not set)
        if self.namenode_uri.is_empty() {
            if let Some(addr) = self.raw_hadoop_props.get("dfs.namenode.rpc-address").cloned() {
                if !addr.is_empty() {
                    self.namenode_uri = if addr.starts_with("hdfs://") {
                        addr
                    } else {
                        format!("hdfs://{}", addr)
                    };
                }
            }
        }
    }

    /// Load the default per-user profile from `~/.hfs/profile.env`.
    /// Silently skips if the file does not exist.
    fn merge_from_profile_env(&mut self) {
        let home = match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            Ok(h) => h,
            Err(_) => return,
        };
        let path = PathBuf::from(home).join(".hfs").join("profile.env");
        self.merge_from_env_file(&path);
    }

    /// Parse a .env profile file and merge into this config.
    ///
    /// Format: `KEY=VALUE` lines (shell-style). Values may be quoted with `"` or `'`.
    /// Lines starting with `#` and blank lines are ignored.
    ///
    /// Supported keys:
    ///   HFS_NAMENODE      — NameNode address (same formats as --namenode)
    ///   HFS_USER          — HDFS user for simple auth (alias: HADOOP_USER_NAME)
    ///   HFS_BACKEND       — rpc | webhdfs | auto
    ///   HADOOP_CONF_DIR   — path to Hadoop client config dir (loads all XML files)
    ///   KRB5_PRINCIPAL    — Kerberos principal (e.g. hdfs/nn.corp@REALM)
    ///   KRB5_KEYTAB       — path to keytab file
    ///
    /// Fields already set by higher-priority sources are NOT overwritten.
    /// Silently skips if the file does not exist or is unreadable.
    pub fn merge_from_env_file(&mut self, path: &Path) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, raw_value)) = line.split_once('=') {
                let key = key.trim();
                // Strip optional surrounding quotes from the value
                let value = raw_value.trim().trim_matches('"').trim_matches('\'');
                self.apply_env_key(key, value);
            }
        }
    }

    /// Apply a single KEY=VALUE pair from an env file or environment variable.
    fn apply_env_key(&mut self, key: &str, value: &str) {
        match key {
            "HFS_NAMENODE" => {
                if self.webhdfs_url.is_none() && self.namenode_uri.is_empty() {
                    self.apply_namenode_str(value);
                }
            }
            "HFS_USER" | "HADOOP_USER_NAME" => {
                if self.hdfs_user.is_none() {
                    self.hdfs_user = Some(value.to_string());
                }
            }
            "HFS_BACKEND" => {
                if self.preferred_backend == "auto" {
                    self.preferred_backend = value.to_string();
                }
            }
            "HADOOP_CONF_DIR" => {
                // Load all Hadoop XML config files from this directory.
                // Only if not already loaded from /etc/hadoop/conf.
                if self.raw_hadoop_props.is_empty() {
                    self.load_hadoop_conf_dir(&PathBuf::from(value));
                }
            }
            "KRB5_PRINCIPAL" => {
                if self.kerberos_principal.is_none() {
                    self.kerberos_principal = Some(value.to_string());
                }
            }
            "KRB5_KEYTAB" => {
                if self.keytab_path.is_none() {
                    self.keytab_path = Some(PathBuf::from(value));
                }
            }
            _ => {} // unknown key — ignore
        }
    }

    /// Parse a core-site.xml file and merge relevant properties.
    pub fn merge_from_core_site(&mut self, path: &PathBuf) -> Result<(), HfsError> {
        let xml = std::fs::read_to_string(path)
            .map_err(|e| HfsError::Config(format!("cannot read {}: {}", path.display(), e)))?;

        let props = parse_hadoop_xml(&xml).map_err(|e| {
            HfsError::Config(format!("XML parse error in {}: {}", path.display(), e))
        })?;

        if let Some(v) = props
            .get("fs.defaultFS")
            .or_else(|| props.get("fs.default.name"))
        {
            self.namenode_uri = v.clone();
        }
        if let Some(v) = props.get("dfs.namenode.http-address") {
            if !v.starts_with("http") {
                self.webhdfs_url = Some(format!("http://{}", v));
            } else {
                self.webhdfs_url = Some(v.clone());
            }
        }

        self.raw_hadoop_props.extend(props);
        Ok(())
    }

    fn merge_from_env(&mut self) {
        // Env vars always override profile.env — so we write directly, not via apply_env_key.
        if let Ok(nn) = std::env::var("HFS_NAMENODE") {
            self.apply_namenode_str(&nn);
        }
        if let Ok(user) = std::env::var("HFS_USER").or_else(|_| std::env::var("HADOOP_USER_NAME")) {
            self.hdfs_user = Some(user);
        }
        if let Ok(backend) = std::env::var("HFS_BACKEND") {
            self.preferred_backend = backend;
        }
        if let Ok(dir) = std::env::var("HADOOP_CONF_DIR") {
            // HADOOP_CONF_DIR from env overrides profile.env setting
            self.load_hadoop_conf_dir(&PathBuf::from(dir));
        }
    }

    /// Apply a namenode string (from CLI or env) to this config.
    /// Exported for testability without env var manipulation.
    pub fn apply_namenode_str(&mut self, nn: &str) {
        if nn.starts_with("http://") || nn.starts_with("https://") {
            self.webhdfs_url = Some(nn.to_string());
        } else if nn.starts_with("hdfs://") {
            self.namenode_uri = nn.to_string();
        } else {
            // Bare host:port — infer intent from port number.
            // 9870 (Hadoop 3.x WebHDFS) or 50070 (Hadoop 2.x/HDP WebHDFS) → HTTP.
            // 8020/8021 (HDFS RPC) or unknown → namenode_uri so auto probes RPC first.
            let port = nn.split(':').next_back().and_then(|p| p.parse::<u16>().ok());
            match port {
                Some(9870) | Some(50070) => {
                    self.webhdfs_url = Some(format!("http://{}", nn));
                }
                _ => {
                    self.namenode_uri = format!("hdfs://{}", nn);
                }
            }
        }
    }

    /// Derive the effective WebHDFS base URL from the configuration.
    pub fn effective_webhdfs_url(&self) -> String {
        if let Some(ref url) = self.webhdfs_url {
            return url.trim_end_matches('/').to_string();
        }
        let uri = &self.namenode_uri;
        if uri.starts_with("hdfs://") {
            let host_port = uri.trim_start_matches("hdfs://");
            let host = host_port.split(':').next().unwrap_or("localhost");
            return format!("http://{}:9870", host);
        }
        if uri.starts_with("http://") || uri.starts_with("https://") {
            return uri.trim_end_matches('/').to_string();
        }
        // Bare host:port stored in namenode_uri (defensive fallback).
        if !uri.is_empty() && !uri.contains("://") {
            return format!("http://{}", uri.trim_end_matches('/'));
        }
        "http://localhost:9870".to_string()
    }
}

/// Parse a Hadoop XML configuration file (core-site.xml, hdfs-site.xml, etc.)
/// Returns a HashMap of property name -> value.
///
/// Format:
/// ```xml
/// <configuration>
///   <property>
///     <name>fs.defaultFS</name>
///     <value>hdfs://namenode:8020</value>
///   </property>
/// </configuration>
/// ```
pub fn parse_hadoop_xml(xml: &str) -> Result<HashMap<String, String>, String> {
    let mut reader = Reader::from_str(xml);

    let mut props = HashMap::new();
    let mut current_name: Option<String> = None;
    let mut current_value: Option<String> = None;
    let mut in_name = false;
    let mut in_value = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"property" => {
                    current_name = None;
                    current_value = None;
                    in_name = false;
                    in_value = false;
                }
                b"name" => in_name = true,
                b"value" => in_value = true,
                _ => {}
            },
            Ok(Event::Text(e)) => {
                let text = e.unescape().map_err(|e| e.to_string())?.trim().to_string();
                if in_name {
                    current_name = Some(text);
                } else if in_value {
                    current_value = Some(text);
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"name" => in_name = false,
                b"value" => in_value = false,
                b"property" => {
                    if let (Some(k), Some(v)) = (current_name.take(), current_value.take()) {
                        props.insert(k, v);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
    }

    Ok(props)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<configuration>
  <property>
    <name>fs.defaultFS</name>
    <value>hdfs://namenode:8020</value>
  </property>
  <property>
    <name>hadoop.security.authentication</name>
    <value>simple</value>
  </property>
  <property>
    <name>dfs.replication</name>
    <value>3</value>
  </property>
</configuration>"#;

    #[test]
    fn test_parse_hadoop_xml_minimal() {
        let props = parse_hadoop_xml(MINIMAL_XML).expect("should parse");
        assert_eq!(
            props.get("fs.defaultFS").map(String::as_str),
            Some("hdfs://namenode:8020")
        );
        assert_eq!(
            props
                .get("hadoop.security.authentication")
                .map(String::as_str),
            Some("simple")
        );
        assert_eq!(props.get("dfs.replication").map(String::as_str), Some("3"));
    }

    #[test]
    fn test_parse_hadoop_xml_empty_config() {
        let xml = "<configuration></configuration>";
        let props = parse_hadoop_xml(xml).expect("should parse empty config");
        assert!(props.is_empty());
    }

    #[test]
    fn test_effective_webhdfs_url_from_hdfs_uri() {
        let cfg = HdfsConfig {
            namenode_uri: "hdfs://namenode.corp.com:8020".to_string(),
            ..Default::default()
        };
        assert_eq!(cfg.effective_webhdfs_url(), "http://namenode.corp.com:9870");
    }

    #[test]
    fn test_effective_webhdfs_url_bare_host_port() {
        // Bare host:port in namenode_uri (defensive fallback path)
        let cfg = HdfsConfig {
            namenode_uri: "namenode.corp.com:50070".to_string(),
            ..Default::default()
        };
        assert_eq!(
            cfg.effective_webhdfs_url(),
            "http://namenode.corp.com:50070"
        );
    }

    #[test]
    fn test_apply_namenode_rpc_port_sets_namenode_uri() {
        // host:8020 (RPC port) must set namenode_uri, not webhdfs_url,
        // so auto-detect can probe RPC first.
        let mut cfg = HdfsConfig::default();
        cfg.apply_namenode_str("namenode.corp.com:8020");
        assert_eq!(cfg.namenode_uri, "hdfs://namenode.corp.com:8020");
        assert!(cfg.webhdfs_url.is_none());
    }

    #[test]
    fn test_apply_namenode_webhdfs_port_sets_url() {
        // host:9870 must set webhdfs_url directly.
        let mut cfg = HdfsConfig::default();
        cfg.apply_namenode_str("namenode.corp.com:9870");
        assert_eq!(
            cfg.webhdfs_url.as_deref(),
            Some("http://namenode.corp.com:9870")
        );
        assert!(cfg.namenode_uri.is_empty());
    }

    #[test]
    fn test_apply_namenode_hdp2_port_sets_url() {
        // host:50070 (HDP2 WebHDFS) must set webhdfs_url.
        let mut cfg = HdfsConfig::default();
        cfg.apply_namenode_str("namenode.corp.com:50070");
        assert_eq!(
            cfg.webhdfs_url.as_deref(),
            Some("http://namenode.corp.com:50070")
        );
    }

    #[test]
    fn test_effective_webhdfs_url_explicit_override() {
        let cfg = HdfsConfig {
            namenode_uri: "hdfs://namenode:8020".to_string(),
            webhdfs_url: Some("http://namenode:9870".to_string()),
            ..Default::default()
        };
        assert_eq!(cfg.effective_webhdfs_url(), "http://namenode:9870");
    }

    #[test]
    fn test_merge_from_core_site_reads_namenode() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().expect("tmp file");
        tmp.write_all(MINIMAL_XML.as_bytes()).expect("write");
        let mut cfg = HdfsConfig::default();
        cfg.merge_from_core_site(&tmp.path().to_path_buf())
            .expect("merge should succeed");
        assert_eq!(cfg.namenode_uri, "hdfs://namenode:8020");
    }

    #[test]
    fn test_merge_from_env_file_sets_namenode_and_user() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().expect("tmp file");
        writeln!(tmp, "# comment").unwrap();
        writeln!(tmp, "HFS_NAMENODE=hdfs://nn.test:8020").unwrap();
        writeln!(tmp, "HFS_USER=testuser").unwrap();
        writeln!(tmp, "HFS_BACKEND=rpc").unwrap();
        let mut cfg = HdfsConfig::default();
        cfg.merge_from_env_file(tmp.path());
        assert_eq!(cfg.namenode_uri, "hdfs://nn.test:8020");
        assert_eq!(cfg.hdfs_user.as_deref(), Some("testuser"));
        assert_eq!(cfg.preferred_backend, "rpc");
    }

    #[test]
    fn test_merge_from_env_file_hadoop_conf_dir_loads_xml() {
        use std::fs;
        use std::io::Write;
        // Write a temp core-site.xml in a temp dir
        let dir = tempfile::tempdir().expect("tmp dir");
        let xml_path = dir.path().join("core-site.xml");
        fs::write(&xml_path, MINIMAL_XML).expect("write xml");

        // Write a .env file referencing the temp dir
        let mut env_file = tempfile::NamedTempFile::new().expect("tmp env file");
        writeln!(env_file, "HADOOP_CONF_DIR={}", dir.path().display()).unwrap();

        let mut cfg = HdfsConfig::default();
        cfg.merge_from_env_file(env_file.path());
        // HADOOP_CONF_DIR in env file should load core-site.xml → namenode_uri
        assert_eq!(cfg.namenode_uri, "hdfs://namenode:8020");
    }

    #[test]
    fn test_merge_from_env_file_quoted_values() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().expect("tmp file");
        writeln!(tmp, r#"HFS_USER="myuser""#).unwrap();
        writeln!(tmp, "HFS_BACKEND='webhdfs'").unwrap();
        let mut cfg = HdfsConfig::default();
        cfg.merge_from_env_file(tmp.path());
        assert_eq!(cfg.hdfs_user.as_deref(), Some("myuser"));
        assert_eq!(cfg.preferred_backend, "webhdfs");
    }

    #[test]
    fn test_effective_user_defaults_to_hdfs() {
        let cfg = HdfsConfig::default();
        assert_eq!(cfg.effective_user(), "hdfs");
    }

    #[test]
    fn test_effective_user_returns_configured() {
        let cfg = HdfsConfig {
            hdfs_user: Some("davide".to_string()),
            ..Default::default()
        };
        assert_eq!(cfg.effective_user(), "davide");
    }
}
