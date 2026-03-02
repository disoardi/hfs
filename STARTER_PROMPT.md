# hfs — Starter Prompt per Claude Code

> Copia il prompt del giorno corrente come primo messaggio in Claude Code
> dalla directory `~/Progetti/hfs/`.
> Leggi sempre `CLAUDE.md` e `DEV_FLOW.md` prima di iniziare.

---

## Prompt Giorno 1 — WebHDFS backend + comandi base

```
Stiamo sviluppando `hfs`, una CLI Rust per HDFS senza JVM. Leggi CLAUDE.md
prima di fare qualsiasi cosa.

Oggi: Giorno 1 — implementare il WebHDFS backend e i comandi ls/stat/du.

Obiettivi:
1. Creare hfs-core/src/webhdfs.rs con WebHdfsClient che implementa HdfsClient trait
   - list(): GET /webhdfs/v1{path}?op=LISTSTATUS
   - stat(): GET /webhdfs/v1{path}?op=GETFILESTATUS
   - content_summary(): GET /webhdfs/v1{path}?op=GETCONTENTSUMMARY
   - read_range(): richiesta HTTP Range (per footer Parquet)
   - health(): JMX endpoint ?qry=Hadoop:service=NameNode,name=FSNamesystemState
   Usa ureq (sincrono), gestisci errori HTTP come HfsError

2. Completare config.rs: parsing core-site.xml con quick-xml
   - Leggi fs.defaultFS → namenode_uri
   - Leggi hadoop.security.authentication → kerberos | simple

3. Implementare comandi ls/stat/du in hfs/src/main.rs usando WebHdfsClient
   - output text: tabella con comfy-table
   - output json: serde_json::to_string_pretty

4. Test con mockito in tests/:
   - mock GET /webhdfs/v1/?op=LISTSTATUS → fixture JSON
   - verifica che WebHdfsClient::list() ritorni Vec<FileStatus> corretto

Constraint:
- Zero unwrap() nel codice non-test
- cargo clippy -- -D warnings deve passare
- Testa con: HFS_NAMENODE=http://localhost:9870 cargo run --bin hfs -- ls /
```

---

## Prompt Giorno 2 — RPC backend via hdfs-native

```
Continua sviluppo hfs. Leggi CLAUDE.md.

Oggi: Giorno 2 — RPC backend nativo via crate hdfs-native.

Obiettivi:
1. Creare hfs-core/src/rpc.rs con RpcClient che implementa HdfsClient
   - Wrappa hdfs_native::HdfsClient
   - Implementa list(), stat(), content_summary(), read_range()
   - Gestisci errori hdfs-native → HfsError

2. Creare hfs-core/src/builder.rs con HdfsClientBuilder
   - build() → prova RpcClient (connessione TCP su porta 8020)
   - Se connessione rifiutata o timeout → fallback a WebHdfsClient
   - Se --backend forzato → salta auto-detection

3. Aggiornare hfs/src/main.rs per usare HdfsClientBuilder invece di
   WebHdfsClient diretto

4. Aggiungere --show-backend flag: stampa quale backend è stato selezionato

Test manuale su cluster reale:
  cargo run --bin hfs -- --show-backend --namenode hdfs://nn:8020 ls /user

Se hdfs-native manca di qualche operazione o ha un bug:
- Documentalo con un TODO nel codice
- Usa WebHdfsClient come fallback per quella specifica operazione
- Segui DEV_FLOW.md per aprire issue/PR upstream in un secondo momento
```

---

## Prompt Giorno 3 — Schema Parquet (footer reading)

```
Continua sviluppo hfs. Leggi CLAUDE.md.

Oggi: Giorno 3 — lettura schema e statistiche dal footer Parquet.

Obiettivi:
1. Implementare HdfsRangeReader in hfs-schema/src/lib.rs
   - Wrappa HdfsClient::read_range()
   - Implementa SeekableReader trait
   - Test con LocalFileReader su file Parquet locale

2. Completare hfs-schema/src/parquet.rs: ParquetInspector::inspect()
   - Leggi ultimi 8 byte → verifica magic PAR1 → footer_len
   - Range request degli ultimi (footer_len + 8) byte
   - Usa crate parquet (Apache Arrow) per deserializzare FileMetaData
   - Converti schema Arrow → Schema unificato hfs
   - Estrai column stats (min/max/null_count) da RowGroup metadata

3. Implementare comandi in hfs/src/main.rs:
   - hfs schema /path/file.parquet → stampa schema ad albero
   - hfs stats /path/file.parquet → tabella colonne con statistiche
   - hfs rowcount /path/ → somma row_count da tutti i Parquet nel path

4. Test:
   - Scarica un file Parquet piccolo di test (~1MB) e mettilo in tests/fixtures/
   - Verifica schema == atteso senza leggere i dati del file
   - Verifica che read_range faccia solo una richiesta HTTP (non scarica tutto)

Il trick: per un file Parquet da 500MB, leggiamo solo ~50KB. Verifica che
il test con mockito lanci esattamente 2 richieste HTTP (tail + footer).
```

---

## Prompt Giorno 4 — blocks/replicas/health + config

```
Continua sviluppo hfs. Leggi CLAUDE.md.

Oggi: Giorno 4 — block inspection, cluster health, config auto-detect.

Obiettivi:
1. Implementare hfs blocks/replicas:
   - WebHDFS: GET ?op=GETFILEBLOCKLOCATIONS → lista blocchi con DataNode locations
   - RPC: hdfs-native block locations API
   - hfs replicas: filega file con replication < dfs.replication.min

2. Completare config.rs: parsing core-site.xml
   - quick-xml per leggere property name/value
   - Mappa fs.defaultFS, dfs.namenode.rpc-address, hadoop.security.authentication
   - Test: crea un core-site.xml minimale in tests/ e verifica parsing

3. Implementare hfs health:
   - JMX NameNode: NumLiveDataNodes, NumDeadDataNodes, CapacityUsed, CapacityTotal
   - NameNode HA state: active/standby
   - Under-replicated blocks count

4. Implementare hfs small-files /path --threshold 128M:
   - Content summary di ogni subdirectory
   - avg_file_size = space_consumed / file_count
   - Stampa directory con avg < threshold, ordinate per file_count desc

5. hfs find con filtri -mtime e -size:
   - Traverse ricorsivo con stat()
   - Filtri: mtime (-N = ultimi N giorni, +N = più vecchi di N giorni)
   - Filtri: size (+NM = più grande di NM, -NM = più piccolo)
```

---

## Prompt Giorno 5 — Schema drift vs Hive + packaging

```
Continua sviluppo hfs. Leggi CLAUDE.md e DEV_FLOW.md.

Oggi: Giorno 5 — Hive Metastore integration, drift detection, release MVP.

Obiettivi:
1. Implementare hfs-schema/src/hive.rs: HiveMetastoreClient
   - Connessione Thrift TCP porta 9083
   - GetTable(db, table) → FieldSchema[] → Schema unificato
   - Se HMS 3.0+: prova prima HTTP API (GET /api/v1/database/{db}/table/{table})
   - Fallback a Thrift se HTTP non disponibile

2. Implementare hfs drift /path --against hive://db.table:
   - Leggi schema di tutti i file Parquet nel path (usa ParquetInspector)
   - Leggi schema tabella Hive (HiveMetastoreClient)
   - SchemaDiff::compare() per ogni file vs Hive
   - Output: lista di deviazioni, colori per breaking/non-breaking

3. Implementare hfs schema /path --against hive://db.table:
   - Stessa logica di drift ma per singolo path/file
   - Output: side-by-side se solo un file, summary se directory

4. Packaging:
   - tuxbox.toml con entrypoint = "hfs" e isolation = "none" (binary)
   - Cross-compile: cargo build --release --target x86_64-unknown-linux-musl
   - Verifica dimensione binario (target: <15MB dopo strip)
   - Tag v0.1.0

5. Valuta cosa ha senso proporre upstream a datafusion-hdfs-native:
   - Stai leggendo DEV_FLOW.md per il processo
   - Candidati: read_range, core-site.xml parsing, qualsiasi bug trovato
   - Apri issues (non ancora PR) per iniziare l'engagement con la community
```

---

## Prompt per sessione di patch upstream

```
Oggi lavoriamo su una contribuzione upstream a datafusion-hdfs-native.
Leggi CLAUDE.md e DEV_FLOW.md prima di tutto.

Il problema da risolvere in hdfs-native è: [DESCRIZIONE DEL BUG/FEATURE]

Procedura:
1. Verifica che il [patch.crates-io] sia su un branch feat/upstream-* (MAI su main)
2. Implementa la fix nel clone locale di datafusion-hdfs-native
3. Scrivi test che dimostrano il problema e la soluzione
4. Testa via hfs su cluster reale
5. Prepara un commit pulito (niente riferimenti a hfs o AI)
6. Segui DEV_FLOW.md sezione "Ciclo upstream" per aprire la PR

Il commit message deve essere in inglese, descrivere il problema tecnico,
e seguire il formato dei commit esistenti nel repo upstream.
```

---

## Note generali

- Sempre `cargo clippy -- -D warnings` prima di committare
- Sempre `cargo test --workspace` prima di merge in main
- Verificare `grep -r "patch.crates-io" Cargo.toml` prima di release
- Log di debug: `RUST_LOG=debug cargo run --bin hfs -- <comando>`
- Documentazione API: `cargo doc --open`
