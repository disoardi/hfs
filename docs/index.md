# hfs — HDFS CLI senza JVM

**hfs** è uno strumento a riga di comando scritto in Rust per interagire con cluster HDFS
senza richiedere Java Virtual Machine né client Hadoop installati.

## Perché hfs?

Il flusso tipico di debug su HDFS oggi richiede 4-6 comandi JVM separati, ciascuno
con 4-8 secondi di startup. Con `hfs`, tutta l'informazione necessaria arriva in meno di 200ms.

```bash
# Prima: 5 comandi JVM, ~30 secondi
hdfs dfs -ls /data/warehouse/transactions/
hdfs fsck /data/warehouse/transactions/ -files -blocks
hadoop jar parquet-tools.jar schema hdfs://nn:8020/data/.../part-00001.parquet
hive -e "describe formatted transactions;"
hdfs dfs -ls -t /data/warehouse/transactions/ | head -5

# Dopo: 1 comando, <2 secondi
hfs inspect /data/warehouse/transactions/ --against hive://default.transactions
```

## Funzionalità principali

- **Filesystem** — `ls`, `du`, `stat`, `find`, `blocks`, `replicas`, `small-files`
- **Schema** — lettura schema Parquet/Avro/ORC leggendo solo ~8KB (footer/header)
- **Drift detection** — confronto schema file vs tabella Hive Metastore
- **Health** — metriche cluster NameNode via JMX
- **Dual backend** — protocollo RPC nativo (hdfs-native) con fallback automatico a WebHDFS
- **Kerberos** — supporto autenticazione enterprise (feature flag)
- **Output** — testo tabellare o JSON (`--json`)

## Installazione rapida

=== "Linux x86_64"

    ```bash
    curl -L https://github.com/disoardi/hfs/releases/latest/download/hfs-linux-x86_64 \
      -o /usr/local/bin/hfs && chmod +x /usr/local/bin/hfs
    ```

=== "macOS ARM64"

    ```bash
    curl -L https://github.com/disoardi/hfs/releases/latest/download/hfs-macos-arm64 \
      -o /usr/local/bin/hfs && chmod +x /usr/local/bin/hfs
    ```

=== "Da sorgente"

    ```bash
    cargo install --git https://github.com/disoardi/hfs hfs
    ```

## Guida rapida

```bash
# Lista file
hfs ls /data/warehouse/

# Schema di un file Parquet
hfs schema /data/warehouse/part-00001.parquet

# Salute del cluster
hfs health

# Drift schema vs tabella Hive
hfs drift /data/warehouse/ --against hive://default.transactions

# Output JSON per script/pipeline
hfs ls /data/ --json | jq '.[] | select(.replication < 3)'
```

Consulta la sezione [Comandi](comandi.md) per la documentazione completa.
