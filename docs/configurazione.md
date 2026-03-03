# Configurazione

## Priority chain

hfs legge la configurazione nell'ordine seguente (priorità decrescente):

1. Flag CLI (`--namenode`, `--backend`)
2. Variabili d'ambiente (`HFS_NAMENODE`, `HFS_BACKEND`, `HFS_USER`)
3. `$HADOOP_CONF_DIR/core-site.xml`
4. `/etc/hadoop/conf/core-site.xml`
5. Default: `http://localhost:9870` (WebHDFS), utente corrente

---

## Flag `--backend`

```bash
hfs --backend auto     # default: prova RPC, fallback WebHDFS
hfs --backend webhdfs  # forza WebHDFS REST (porta 9870)
hfs --backend rpc      # forza RPC nativo (porta 8020)
```

### Logica di auto-detection

Con `--backend auto` (default):

1. Se `HFS_NAMENODE` inizia con `hdfs://` (es. `hdfs://namenode:8020`):
   - Costruisce il client RPC (nessuna connessione ancora)
   - Prova `stat /` con timeout 3 secondi
   - Se risponde → usa **RPC** (porta 8020, protocollo nativo)
   - Se timeout o rifiuta la connessione → usa **WebHDFS** (porta 9870)
2. Altrimenti → usa **WebHDFS** direttamente

### Differenze tra backend

| Feature | `rpc` | `webhdfs` |
|---------|-------|-----------|
| Protocollo | Protobuf/TCP porta 8020 | HTTP/REST porta 9870 |
| Startup | immediato | immediato |
| `ls`, `stat` | ✅ | ✅ |
| `du` | ❌ (non disponibile in hdfs-native v0.9) | ✅ |
| `health` | ❌ (richiede JMX) | ✅ |
| `schema`, `rowcount` | ✅ via `read_range` | ✅ via range request |
| Kerberos RPC | ✅ con `--features kerberos` | ❌ |
| Kerberos WebHDFS (SPNEGO) | ❌ | ✅ via `kinit` |

!!! note "Limitazione hdfs-native v0.9"
    Il backend RPC non espone `replication` e `block_size` in `ls`/`stat`.
    Questi campi mostrano `-` o sono omessi. Sarà risolto in una PR upstream.

---

## Variabili d'ambiente

| Variabile | Descrizione | Esempio |
|-----------|-------------|---------|
| `HFS_NAMENODE` | URI del NameNode. `http://` → WebHDFS, `hdfs://` → RPC | `hdfs://namenode:8020` |
| `HFS_BACKEND` | Backend: `rpc`, `webhdfs`, `auto` | `webhdfs` |
| `HFS_USER` | Utente HDFS (sovrascrive `HADOOP_USER_NAME`) | `hadoop` |
| `HADOOP_CONF_DIR` | Directory configurazione Hadoop | `/etc/hadoop/conf` |
| `HADOOP_USER_NAME` | Utente HDFS (compatibilità Hadoop) | `hadoop` |
| `RUST_LOG` | Log level | `debug` |

---

## core-site.xml

Se `HADOOP_CONF_DIR` è impostato o `/etc/hadoop/conf/core-site.xml` esiste,
hfs legge automaticamente queste proprietà:

| Proprietà | Descrizione |
|-----------|-------------|
| `fs.defaultFS` | NameNode URI principale (es. `hdfs://namenode:8020`) |
| `fs.default.name` | Alias legacy di `fs.defaultFS` |
| `dfs.namenode.http-address` | URL WebHDFS esplicito (es. `namenode:9870`) |

**Esempio `core-site.xml` minimale:**
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

## Diagnostica

```bash
# Verifica quale backend è stato selezionato
hfs --show-backend ls /

# Forza il backend e vedi la risposta
hfs --backend webhdfs --show-backend capabilities
hfs --backend rpc --show-backend capabilities
```

Output atteso su un cluster con WebHDFS:
```
[backend: WebHDFS]
hfs v0.1.0-dev
Backend: WebHDFS
Kerberos: disabled (build with --features kerberos for RPC Kerberos)
WebHDFS URL: http://namenode:9870
NameNode URI: hdfs://namenode:8020
```
