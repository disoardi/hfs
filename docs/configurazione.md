# Configurazione

!!! tip "Stub"
    Questa pagina verrà completata durante lo sprint di sviluppo.

## Priority chain

hfs legge la configurazione nell'ordine seguente (priorità decrescente):

1. Flag CLI (`--namenode`, `--backend`, `--user`)
2. Variabili d'ambiente (`HFS_NAMENODE`, `HFS_BACKEND`, `HFS_USER`)
3. File `~/.hfs/config.toml`
4. `$HADOOP_CONF_DIR/core-site.xml`
5. `/etc/hadoop/conf/core-site.xml`
6. Default: `http://localhost:9870` (WebHDFS), utente corrente

## File di configurazione `~/.hfs/config.toml`

```toml
namenode = "hdfs://namenode.prod.internal:8020"
webhdfs_url = "http://namenode.prod.internal:9870"
kerberos = true
hive_metastore_url = "http://hive-metastore.prod.internal:8080"
```

## Variabili d'ambiente

| Variabile | Descrizione |
|-----------|-------------|
| `HFS_NAMENODE` | URI del NameNode |
| `HFS_BACKEND` | Backend: `rpc`, `webhdfs`, `auto` |
| `HFS_USER` | Utente HDFS |
| `HFS_HMS_URL` | URL Hive Metastore (HTTP API) |
| `HFS_KERBEROS` | `true` per abilitare autenticazione Kerberos |
| `HADOOP_CONF_DIR` | Directory configurazione Hadoop |
| `RUST_LOG` | Log level (es. `debug`, `info`) |
