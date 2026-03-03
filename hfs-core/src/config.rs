// HdfsConfig — configuration with automatic core-site.xml reading
//
// Priority (lowest to highest):
//   1. Built-in defaults (localhost:9870)
//   2. /etc/hadoop/conf/core-site.xml
//   3. $HADOOP_CONF_DIR/core-site.xml
//   4. Environment variables (HFS_NAMENODE, HFS_USER, HFS_BACKEND)
//   5. CLI flags via merge_cli()

use crate::error::HfsError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::PathBuf;

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
    pub fn load() -> Result<Self, HfsError> {
        let mut cfg = Self::default();

        for path in Self::hadoop_conf_candidates() {
            if path.exists() {
                cfg.merge_from_core_site(&path)?;
                break;
            }
        }

        cfg.merge_from_env();

        if cfg.namenode_uri.is_empty() && cfg.webhdfs_url.is_none() {
            cfg.namenode_uri = "hdfs://localhost:8020".to_string();
        }

        Ok(cfg)
    }

    fn hadoop_conf_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        candidates.push(PathBuf::from("/etc/hadoop/conf/core-site.xml"));
        if let Ok(dir) = std::env::var("HADOOP_CONF_DIR") {
            candidates.push(PathBuf::from(dir).join("core-site.xml"));
        }
        candidates
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
        if let Ok(nn) = std::env::var("HFS_NAMENODE") {
            if nn.starts_with("http://") || nn.starts_with("https://") {
                self.webhdfs_url = Some(nn);
            } else if nn.starts_with("hdfs://") {
                self.namenode_uri = nn;
            } else {
                // Bare host:port — treat as WebHDFS (port 50070 = Hadoop 2.x, 9870 = Hadoop 3.x).
                self.webhdfs_url = Some(format!("http://{}", nn));
            }
        }
        if let Ok(user) = std::env::var("HFS_USER").or_else(|_| std::env::var("HADOOP_USER_NAME")) {
            self.hdfs_user = Some(user);
        }
        if let Ok(backend) = std::env::var("HFS_BACKEND") {
            self.preferred_backend = backend;
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
}
