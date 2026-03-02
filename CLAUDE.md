# hfs — CLAUDE.md

Leggimi per intero prima di toccare il codice.

---

## Cos'è hfs

CLI Rust per interagire con HDFS **senza JVM, senza client Hadoop**.
Fa due cose che non esistono insieme altrove:
- Parla il protocollo HDFS nativo (via `hdfs-native`, no JVM)
- Legge schema e statistiche da file Parquet/Avro/ORC leggendo solo footer/header (O(KB) non O(MB))

**Tagline:** `hfs ls /path` invece di `hdfs dfs -ls /path`. Startup 50ms invece di 4 secondi.

---

## Architettura — Workspace Cargo

```
hfs/                   ← questo repo
├── Cargo.toml         ← workspace root, sezione [patch.crates-io] commentata
├── hfs-core/          ← libreria: HdfsClient trait, RpcClient, WebHdfsClient, config
├── hfs-schema/        ← libreria: ParquetInspector, AvroInspector, OrcInspector, HiveMetastoreClient
└── hfs/               ← binario CLI (clap)
```

### hfs-core

`HdfsClient` trait con due implementazioni:
- `RpcClient`: wrappa `hdfs-native`, porta 8020, protocollo RPC con Protobuf
- `WebHdfsClient`: HTTP via `ureq`, porta 9870, API REST WebHDFS

`HdfsClientBuilder` seleziona automaticamente: prova RPC → se timeout/refused → WebHDFS.
Forzare con `--backend rpc|webhdfs`.

**Metodo chiave per schema inspection:**
```rust
// Usato da hfs-schema per leggere solo il footer Parquet
async fn read_range(&self, path: &str, offset: u64, length: u64) -> Result<Vec<u8>>
```

### hfs-schema

`SeekableReader` trait → implementato da `HdfsRangeReader` (wrappa `HdfsClient::read_range`)
e da `LocalFileReader` (per test su file locali).

`ParquetInspector::inspect()` → legge ultimi ~8KB, deserializza footer, ritorna `ParquetMeta`.
`AvroInspector::inspect()` → legge primi ~4KB, ritorna `Schema`.
`HiveMetastoreClient` → Thrift API su porta 9083, ritorna schema tabella.

`SchemaDiff::compare(a, b)` → diff tra due `Schema` qualsiasi (Parquet vs Hive, ecc.).

---

## Regola DEI DUE LAYER — Fondamentale

### Layer 1 — Interno (questo repo, privato)
Tutta la logica di `hfs`. Dipende da `hdfs-native` via crates.io.

Quando trovi un bug o gap in `hdfs-native` durante lo sviluppo:
1. Fixalo nel clone locale di `datafusion-hdfs-native` (in `../datafusion-hdfs-native`)
2. Attiva `[patch.crates-io]` in `Cargo.toml` per testare
3. Verifica che funzioni su cluster reale
4. Vedi DEV_FLOW.md per come aprire PR upstream

**IMPORTANTE:** il `[patch.crates-io]` va su branch `feat/fix-*`, MAI su `main`.
Controlla sempre prima di fare release che `Cargo.toml` non abbia patch attive.

### Layer 2 — Upstream (`disoardi/datafusion-hdfs-native`, fork pubblico)
Modifiche generiche che hanno senso per chiunque usi hdfs-native.
Vedi `DEV_FLOW.md` sezione "Ciclo upstream" per il flusso completo.

**Cosa va upstream vs resta in hfs:**

| Modifica | Dove |
|----------|------|
| Bug nel protocollo RPC | upstream hdfs-native |
| Supporto Hadoop 2.x | upstream hdfs-native |
| Parsing core-site.xml | upstream hdfs-native |
| read_range / pread | upstream hdfs-native (se manca) |
| Kerberos SASL migliorato | upstream hdfs-native |
| Schema inspection Parquet | hfs-schema (specifico) |
| Hive Metastore Thrift | hfs-schema (specifico) |
| CLI, comandi, output | hfs (mai upstream) |

---

## Regole di Codice — NON Derogabili

- **Rust edition 2021**, stable toolchain
- **Zero unsafe** nel codice applicativo (hfs-core, hfs-schema, hfs)
- **Ogni `Result` gestito** — no `.unwrap()` in codice non-test, usa `?` o `anyhow!`
- **`HdfsClient::run()` non crasha mai** — errori di rete → `Err(HfsError::Connection(...))`
- **Tests** — ogni modulo pubblico ha unit test, fixtures in `tests/fixtures/`
  - Test hfs-core: mock WebHDFS con `mockito`
  - Test hfs-schema: file Parquet/Avro/ORC reali piccoli in `tests/fixtures/`
  - Test hfs-schema Hive: mock Thrift server
- **No tokio::time::sleep nei test** — usa `tokio-test`
- **Clippy pulito**: `cargo clippy -- -D warnings` deve passare senza errori

---

## Comandi Implementati per Sprint

### Giorno 1 — WebHDFS backend + ls/stat/du
File da modificare: `hfs-core/src/webhdfs.rs` (creare), `hfs/src/main.rs`
```
cargo test -p hfs-core
hfs --backend webhdfs --namenode http://localhost:9870 ls /
```

### Giorno 2 — RPC backend via hdfs-native
File: `hfs-core/src/rpc.rs` (creare)
```
hfs --backend rpc --namenode hdfs://namenode:8020 ls /
```

### Giorno 3 — Schema Parquet (footer reading)
File: `hfs-schema/src/parquet.rs`
```
hfs schema /path/file.parquet
hfs stats /path/file.parquet
hfs rowcount /path/
```

### Giorno 4 — blocks/replicas/health + config auto-detect
File: `hfs-core/src/config.rs` (completare), `hfs-core/src/webhdfs.rs` (blocks via JMX)
```
hfs blocks /path/file
hfs health
hfs --namenode hdfs://nn:8020 ls /  # legge core-site.xml se disponibile
```

### Giorno 5 — Schema drift vs Hive + packaging
File: `hfs-schema/src/hive.rs`, `hfs/src/main.rs` (comandi drift/schema --against)
```
hfs drift /path --against hive://default.transactions
hfs schema /path/ --against hive://default.transactions
```

---

## Test senza cluster HDFS reale

```bash
# Unit test con mock (no cluster)
cargo test -p hfs-core
cargo test -p hfs-schema

# Integration test con minicluster Docker
docker run -d --name hdfs-test \
  -p 9870:9870 -p 8020:8020 \
  apache/hadoop:3.3.6 \
  sh -c "hdfs namenode -format && hdfs namenode &  hdfs datanode"

# Test su cluster reale (cluster DXC)
export HFS_NAMENODE=hdfs://namenode.corp.com:8020
cargo run --bin hfs -- --show-backend ls /
```

---

## Dipendenze Chiave

- `hdfs-native` — protocollo RPC HDFS, Apache DataFusion ecosystem
  - Repo upstream: https://github.com/datafusion-contrib/datafusion-hdfs-native
  - Fork personale: https://github.com/disoardi/datafusion-hdfs-native
  - In caso di patch: vedi `DEV_FLOW.md`
- `parquet` (Apache Arrow) — parsing footer Parquet
- `apache-avro` — parsing header Avro
- `ureq` — HTTP client sincrono per WebHDFS (no tokio overhead nel path principale)
- `clap v4` con `derive` — CLI
- `comfy-table` — tabelle ASCII per output `-l`

---

## Riferimenti

- Documentazione IdeaFlow completa: `~/Progetti/silverbullet/space/Idee/ideas/idea-004-*`
- DEV_FLOW.md — flusso git, branch strategy, ciclo upstream
- STARTER_PROMPT.md — prompt pronti per ogni sessione Claude Code
- Parquet spec: https://parquet.apache.org/docs/file-format/
- WebHDFS REST API: https://hadoop.apache.org/docs/stable/hadoop-project-dist/hadoop-hdfs/WebHDFS.html
- hdfs-native docs: https://docs.rs/hdfs-native
