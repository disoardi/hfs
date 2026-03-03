# Configuration

## Priority chain

hfs reads configuration in the following order (highest to lowest priority):

1. CLI flags (`--namenode`, `--backend`)
2. Environment variables (`HFS_NAMENODE`, `HFS_BACKEND`, `HFS_USER`)
3. `$HADOOP_CONF_DIR/core-site.xml`
4. `/etc/hadoop/conf/core-site.xml`
5. Defaults: `http://localhost:9870` (WebHDFS), current OS user

---

## The `--backend` flag

```bash
hfs --backend auto     # default: try RPC, fall back to WebHDFS
hfs --backend webhdfs  # force WebHDFS REST (port 9870)
hfs --backend rpc      # force native RPC (port 8020)
```

### Auto-detection logic

With `--backend auto` (default):

1. If `HFS_NAMENODE` starts with `hdfs://` (e.g. `hdfs://namenode:8020`):
   - Builds the RPC client (no TCP connection yet)
   - Probes `stat /` with a 3-second timeout
   - If it responds → uses **RPC** (port 8020, native protocol)
   - If timeout or connection refused → uses **WebHDFS** (port 9870)
2. Otherwise → uses **WebHDFS** directly

### Backend comparison

| Feature | `rpc` | `webhdfs` |
|---------|-------|-----------|
| Protocol | Protobuf/TCP port 8020 | HTTP/REST port 9870 |
| `ls`, `stat` | ✅ | ✅ |
| `du` | ❌ (not in hdfs-native v0.9) | ✅ |
| `health` | ❌ (requires JMX) | ✅ |
| `schema`, `rowcount` | ✅ via `read_range` | ✅ via HTTP range request |
| Kerberos RPC | ✅ with `--features kerberos` | ❌ |
| Kerberos WebHDFS (SPNEGO) | ❌ | ✅ via `kinit` |

!!! note "hdfs-native v0.9 limitation"
    The RPC backend does not expose `replication` and `block_size` in `ls`/`stat`.
    These fields show `-` or are omitted. A fix will be contributed upstream.

---

## Environment variables

| Variable | Description | Example |
|----------|-------------|---------|
| `HFS_NAMENODE` | NameNode URI. `http://` → WebHDFS, `hdfs://` → RPC | `hdfs://namenode:8020` |
| `HFS_BACKEND` | Backend: `rpc`, `webhdfs`, `auto` | `webhdfs` |
| `HFS_USER` | HDFS user (overrides `HADOOP_USER_NAME`) | `hadoop` |
| `HADOOP_CONF_DIR` | Hadoop configuration directory | `/etc/hadoop/conf` |
| `HADOOP_USER_NAME` | HDFS user (Hadoop compatibility) | `hadoop` |
| `RUST_LOG` | Log level | `debug` |

---

## core-site.xml

If `HADOOP_CONF_DIR` is set or `/etc/hadoop/conf/core-site.xml` exists,
hfs automatically reads these properties:

| Property | Description |
|----------|-------------|
| `fs.defaultFS` | Primary NameNode URI (e.g. `hdfs://namenode:8020`) |
| `fs.default.name` | Legacy alias for `fs.defaultFS` |
| `dfs.namenode.http-address` | Explicit WebHDFS URL (e.g. `namenode:9870`) |

**Minimal `core-site.xml` example:**
```xml
<configuration>
  <property>
    <name>fs.defaultFS</name>
    <value>hdfs://namenode.prod.internal:8020</value>
  </property>
  <property>
    <name>dfs.namenode.http-address</name>
    <value>namenode.prod.internal:9870</value>
  </property>
</configuration>
```

---

## Diagnostics

```bash
# Check which backend was selected
hfs --show-backend ls /

# Force a backend and inspect the response
hfs --backend webhdfs --show-backend capabilities
hfs --backend rpc --show-backend capabilities
```

Expected output on a cluster with WebHDFS:
```
[backend: WebHDFS]
hfs v0.1.0-dev
Backend: WebHDFS
Kerberos: disabled (build with --features kerberos for RPC Kerberos)
WebHDFS URL: http://namenode:9870
NameNode URI: hdfs://namenode:8020
```
