# hfs — CLAUDE.md

Leggimi per intero prima di toccare qualsiasi file.

---

## Lingua e Stile

- **Interazione con Davide**: sempre in **italiano**
- **Commenti nel codice**: sempre in **inglese**
- **Documentazione (docs/)**: italiano come lingua principale, inglese in `docs/en/`
- **Commit message**: inglese, formato convenzionale (`feat:`, `fix:`, `chore:`, `docs:`)
- **PR upstream datafusion-hdfs-native**: tutto in inglese, nessun riferimento a hfs o AI

---

## Cos'è hfs

CLI Rust per interagire con HDFS **senza JVM, senza client Hadoop installato**.
Due capacità che non esistono insieme altrove:
- Protocollo HDFS nativo (via `hdfs-native`, puro Rust, no JVM)
- Schema inspection Parquet/Avro/ORC leggendo solo footer/header (~8KB su file da 500MB)

**Tagline:** `hfs ls /path` invece di `hdfs dfs -ls /path`. Startup 50ms invece di 4 secondi.

---

## Architettura — Workspace Cargo

```
hfs/
├── Cargo.toml              ← workspace root, [patch.crates-io] sempre commentato su main
├── hfs-core/               ← HdfsClient trait, RpcClient, WebHdfsClient, config
├── hfs-schema/             ← ParquetInspector, AvroInspector, HiveMetastoreClient, SchemaDiff
├── hfs/                    ← binario CLI (clap)
├── docker/                 ← ambienti Docker per test locali e CI
│   ├── docker-compose.test.yml    ← HDFS cluster (namenode + 2 datanode + HMS)
│   ├── docker-compose.kerb.yml   ← come sopra + FreeIPA KDC per Kerberos
│   └── init/                      ← script di inizializzazione cluster
├── docs/                   ← documentazione MkDocs (IT principale, EN in docs/en/)
├── mkdocs.yml
└── .github/
    └── workflows/
        └── docs.yml        ← GitHub Pages deploy automatico su push main
```

---

## Build Rust — Regola di Auto-Contenimento

Il binario `hfs` deve girare su qualsiasi edge node Hadoop **senza installare nulla**.
Questo significa: zero dipendenze da librerie di sistema non-standard.

### Dipendenze HTTP: rustls, non OpenSSL
```toml
# CORRETTO — puro Rust, no libssl
ureq = { version = "2", features = ["tls"] }           # usa rustls internamente

# SBAGLIATO — richiede libssl su sistema
ureq = { version = "2", features = ["native-tls"] }
reqwest = { version = "0.11", features = ["native-tls"] }
```

### XML parsing: quick-xml (puro Rust)
```toml
quick-xml = { version = "0.31", features = ["serialize"] }
```

### Kerberos: feature flag, mai obbligatorio
Kerberos richiede `libgssapi` di sistema. Due modalità di build:

```toml
[features]
default = []
kerberos = ["dep:libgssapi-sys"]
```

Build senza Kerberos (distribuzione standard, musl compatibile):
```bash
cargo build --release --target x86_64-unknown-linux-musl
```

Build con Kerberos (per cluster enterprise con HDFS RPC + SASL):
```bash
cargo build --release --features kerberos
# Non usare musl con kerberos: libgssapi-sys non è musl-friendly
```

WebHDFS con Kerberos (SPNEGO via HTTP) è supportato senza feature flag — usa
il token `Authorization: Negotiate <token>` che viene acquisito tramite `kinit` system call.

### Target di distribuzione
| Target | Kerberos | Uso |
|--------|----------|-----|
| `x86_64-unknown-linux-musl` | ❌ (WebHDFS only) | Distribuzione standard, binario statico <15MB |
| `x86_64-unknown-linux-gnu` + `--features kerberos` | ✅ | Cluster enterprise con HDFS RPC + Kerberos |
| `aarch64-apple-darwin` | ❌ | macOS ARM64 |

### Verifica self-containment
```bash
# Dopo build musl, verifica zero dipendenze dinamiche
ldd target/x86_64-unknown-linux-musl/release/hfs
# Output atteso: "not a dynamic executable" oppure solo linux-vdso/ld-linux
```

---

## Regola DEI DUE LAYER — Fondamentale

### Layer 1 — Interno (repo privato `disoardi/hfs`)
Tutta la logica di `hfs`. Sviluppo veloce, iterazioni frequenti.
Usa `hdfs-native` da crates.io come dipendenza normale.

### Layer 2 — Upstream (`disoardi/datafusion-hdfs-native`, fork pubblico)
Bug fix e feature generici utili a chiunque usi hdfs-native.
NON va upstream: codice specifico per hfs, logica di schema inspection,
integrazione Hive, qualsiasi cosa menzionata solo in hfs.

VA upstream (candidati tipici):
- Bug nel protocollo RPC HDFS
- Supporto Hadoop 2.x / CDP specifici
- `read_range` / `pread` se mancante
- Parsing `core-site.xml` migliorato
- Fix Kerberos SASL token renewal

Vedi `DEV_FLOW.md` per il flusso upstream completo.
**IMPORTANTE:** `[patch.crates-io]` mai attivo su branch `main`.

---

## Infrastruttura di Test

### Tre livelli di test

1. **Unit test** (no cluster, mock HTTP):
   ```bash
   cargo test --workspace
   ```

2. **Integration test HDFS** (cluster Docker locale):
   ```bash
   # Avvia il cluster (namenode + 2 datanode + HMS)
   docker compose -f docker/docker-compose.test.yml up -d
   # Aspetta che sia healthy
   docker compose -f docker/docker-compose.test.yml exec namenode hdfs dfsadmin -report
   # Carica fixture
   docker compose -f docker/docker-compose.test.yml exec namenode \
     hdfs dfs -put /fixtures/ /test-data/
   # Esegui integration test
   HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
   # Teardown
   docker compose -f docker/docker-compose.test.yml down
   ```

3. **Integration test Kerberos** (cluster Docker + FreeIPA):
   ```bash
   docker compose -f docker/docker-compose.kerb.yml up -d
   # Aspetta FreeIPA (lento, ~3 minuti al primo avvio)
   docker compose -f docker/docker-compose.kerb.yml exec freeipa \
     /scripts/wait-for-ipa.sh
   # Ottieni ticket Kerberos
   docker compose -f docker/docker-compose.kerb.yml exec hdfs-client \
     kinit -kt /keytabs/hfs.keytab hfs/hdfs-client
   # Test con Kerberos
   HFS_NAMENODE=hdfs://namenode:8020 HFS_KERBEROS=true \
     cargo test --features kerberos --workspace -- --include-ignored
   # Teardown
   docker compose -f docker/docker-compose.kerb.yml down
   ```

### Comandi per test unit veloci
```bash
# Solo unit test senza cluster
cargo test --workspace

# Test di un singolo crate
cargo test -p hfs-core
cargo test -p hfs-schema

# Test con output verbose
cargo test --workspace -- --nocapture

# Test specifico
cargo test -p hfs-core test_webhdfs_list
```

### Fixture di test
- `tests/fixtures/small.parquet` — file Parquet reale ~500KB per unit test schema
- `tests/fixtures/sample.avro` — file Avro reale ~100KB
- `tests/fixtures/core-site-minimal.xml` — core-site.xml minimale per test config
- `tests/fixtures/core-site-kerb.xml` — core-site.xml con Kerberos per test auth
- Script Docker che popola HDFS con dati di test: `docker/init/load-fixtures.sh`

### Mockito per unit test WebHDFS
```rust
// Ogni test che tocca WebHdfsClient DEVE usare mockito
// Non fare richieste HTTP reali in unit test

#[cfg(test)]
mod tests {
    use mockito::Server;

    #[tokio::test]
    async fn test_list_returns_file_statuses() {
        let mut server = Server::new_async().await;
        let mock = server.mock("GET", "/webhdfs/v1/?op=LISTSTATUS")
            .with_status(200)
            .with_body(include_str!("../tests/fixtures/liststatus.json"))
            .create_async().await;

        let client = WebHdfsClient::new(&server.url());
        let result = client.list("/").await.unwrap();
        assert_eq!(result.len(), 3);
        mock.assert_async().await;  // verifica che la richiesta sia stata fatta
    }
}
```

---

## Regole di Codice — NON Derogabili

- **Rust edition 2021**, toolchain stable (no nightly)
- **Zero `unsafe`** nel codice applicativo (`hfs-core`, `hfs-schema`, `hfs`)
- **Zero `.unwrap()`** fuori dai test — usa sempre `?` o `map_err`
- **`HdfsClient` non panics mai** — errori di rete → `Err(HfsError::Connection(...))`
- **`cargo clippy -- -D warnings`** deve passare senza warning
- **`cargo fmt`** prima di ogni commit
- **Commenti in inglese** — funzioni pubbliche documentate con `///`
- **Test obbligatori**:
  - Ogni funzione pubblica ha almeno un test
  - Ogni caso di errore gestito ha un test che verifica il messaggio di errore
  - I test di schema Parquet verificano che vengano fatte **esattamente N richieste HTTP**
  - Coverage minima: `cargo tarpaulin --workspace` → ≥ 80%

---

## Documentazione — Regole

Ogni PR che cambia comportamento osservabile (comandi, flag, output) **deve**
aggiornare la documentazione corrispondente in `docs/` (IT) e `docs/en/` (EN).

### Struttura docs/
```
docs/
├── index.md              ← Introduzione e quickstart (IT)
├── installazione.md      ← Download, build da source (IT)
├── comandi.md            ← Tutti i comandi con esempi (IT)
├── configurazione.md     ← Config file, env vars, core-site.xml (IT)
├── kerberos.md           ← Setup autenticazione Kerberos (IT)
├── sviluppo.md           ← Come contribuire (IT)
└── en/
    ├── index.md          ← Introduction and quickstart (EN)
    ├── installation.md
    ├── commands.md
    ├── configuration.md
    ├── kerberos.md
    └── development.md
```

### Come aggiornare la documentazione
```bash
# Visualizza docs in locale durante lo sviluppo
pip install mkdocs-material mkdocs-static-i18n
mkdocs serve

# Build locale per verifica
mkdocs build --strict

# Deploy su GitHub Pages (solo su main, di solito via CI)
mkdocs gh-deploy --force
```

La CI (`.github/workflows/docs.yml`) fa il deploy automatico ad ogni push su `main`.

---

## Sprint Plan

### Giorno 0 — Setup infrastruttura (una volta sola)
- [ ] Repo GitHub `disoardi/hfs` creato e push iniziale
- [ ] Fork `datafusion-hdfs-native` creato
- [ ] Docker Compose test cluster funzionante (namenode healthy)
- [ ] MkDocs installato, `mkdocs serve` funziona
- [ ] GitHub Pages attivo su repo pubblico `disoardi/hfs` (branch gh-pages)
- [ ] `.github/workflows/docs.yml` committato e CI verde

### Giorno 1 — WebHDFS backend + ls/stat/du
Files: `hfs-core/src/webhdfs.rs`, `hfs-core/src/config.rs`, `hfs/src/main.rs`
Test: unit test con mockito, integration test su Docker cluster
Docs: aggiorna `docs/comandi.md` con `ls`, `stat`, `du`

### Giorno 2 — RPC backend via hdfs-native + auto-detection
Files: `hfs-core/src/rpc.rs`, `hfs-core/src/builder.rs`
Test: unit test auto-detection fallback, integration test RPC su Docker
Docs: aggiorna `docs/configurazione.md` con `--backend` flag

### Giorno 3 — Schema Parquet (footer reading, ≤2 richieste HTTP)
Files: `hfs-schema/src/lib.rs`, `hfs-schema/src/parquet.rs`
Test: verifica esattamente 2 HTTP request, confronto schema con fixture known-good
Docs: aggiorna `docs/comandi.md` con `schema`, `stats`, `rowcount`

### Giorno 4 — Blocks / replicas / health + config auto-detect
Files: `hfs-core/src/webhdfs.rs` (blocks), `hfs-core/src/config.rs` (complete)
Test: integration test `hfs health` su Docker, verifica parsing core-site.xml
Docs: aggiorna `docs/comandi.md` con `health`, `blocks`, `replicas`, `small-files`

### Giorno 5 — Schema drift vs Hive + packaging + release v0.1.0
Files: `hfs-schema/src/hive.rs`, `hfs/src/main.rs` (drift)
Test: integration test drift con HMS su Docker
Docs: aggiorna tutte le sezioni, verifica mkdocs build --strict, deploy docs

---

## Dipendenze Chiave

| Crate | Feature flag | Note |
|-------|-------------|------|
| `hdfs-native` | — | RPC HDFS, no JVM |
| `ureq` | `tls` (rustls) | HTTP WebHDFS — NO `native-tls` |
| `clap` | `derive` | CLI |
| `parquet` | `arrow`, `async` | Footer reading |
| `apache-avro` | — | Header Avro |
| `serde` + `serde_json` | `derive` | JSON output |
| `comfy-table` | — | Output tabellare |
| `thiserror` | — | Error types |
| `quick-xml` | `serialize` | core-site.xml — NO `libxml2` |
| `tokio` | `rt-multi-thread` | Async runtime |
| `async-trait` | — | HdfsClient trait |
| `libgssapi-sys` | `kerberos` (opt) | GSSAPI per RPC Kerberos |
| `mockito` | dev | Mock HTTP per test |
| `cargo-tarpaulin` | dev | Coverage |

---

## Verifica Pre-Commit

```bash
# Script da eseguire prima di ogni commit (o usare pre-commit hook)
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
# Per integration test (richiede Docker):
# docker compose -f docker/docker-compose.test.yml up -d
# HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
# docker compose -f docker/docker-compose.test.yml down
```

---

## Riferimenti

- DEV_FLOW.md — git workflow, branch strategy, ciclo upstream, GitHub Pages
- STARTER_PROMPT.md — prompt pronti per ogni sessione
- docs/ — documentazione utente (IT/EN)
- Parquet spec: https://parquet.apache.org/docs/file-format/
- WebHDFS API: https://hadoop.apache.org/docs/stable/hadoop-project-dist/hadoop-hdfs/WebHDFS.html
- hdfs-native: https://docs.rs/hdfs-native
- MkDocs Material: https://squidfunk.github.io/mkdocs-material/
