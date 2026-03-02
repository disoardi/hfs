# hfs — Starter Prompt per Claude Code

> Copia il prompt del giorno corrente come primo messaggio in Claude Code.
> Avvia sempre Claude Code dalla directory `~/Progetti/hfs/`.
> Claude Code legge CLAUDE.md automaticamente all'avvio.

---

## Prompt Giorno 0 — Setup infrastruttura (prima sessione in assoluto)

```
Stiamo iniziando lo sviluppo di `hfs`. Leggi CLAUDE.md e DEV_FLOW.md prima di fare qualsiasi cosa.

Oggi: Giorno 0 — Setup completo dell'infrastruttura di sviluppo.

Obiettivi (nell'ordine):

1. Verifica che il repo GitHub disoardi/hfs esista e che il remote origin sia configurato.
   Se manca, crealo con: gh repo create disoardi/hfs --private
   e configura il remote.

2. Verifica fork datafusion-hdfs-native:
   - ls ~/Progetti/datafusion-hdfs-native/ deve esistere
   - git remote -v nel fork deve mostrare sia origin (disoardi) che upstream (datafusion-contrib)
   - Se manca: gh repo fork datafusion-contrib/datafusion-hdfs-native --clone=false
     poi: cd ~/Progetti && git clone git@github.com:disoardi/datafusion-hdfs-native.git
          cd datafusion-hdfs-native && git remote add upstream ...

3. Verifica toolchain Rust:
   - rustup target list --installed | grep x86_64-unknown-linux-musl
   - Se manca: rustup target add x86_64-unknown-linux-musl
   - cargo build --workspace  (deve compilare, anche se ci sono stub/TODO)

4. Avvia cluster HDFS Docker e verifica che funzioni:
   cd ~/Progetti/hfs
   docker compose -f docker/docker-compose.test.yml up -d
   # Aspetta che namenode sia healthy (massimo 2 minuti)
   # Verifica: curl -s http://localhost:9870/jmx?qry=Hadoop:service=NameNode,name=NameNodeStatus | head -5
   # Se il compose fallisce, leggi l'errore e correggilo (es. porta occupata → cambia la porta nel yml)

5. Carica le fixture di test:
   docker exec hfs-test-client /load-fixtures.sh
   # Se manca il file tests/fixtures/small.parquet, scaricane uno da:
   # https://github.com/apache/parquet-testing/raw/master/data/alltypes_plain.parquet
   # e salvalo in tests/fixtures/small.parquet

6. Setup MkDocs:
   python3 -m venv ~/venvs/mkdocs
   source ~/venvs/mkdocs/bin/activate
   pip install mkdocs-material==9.5.* mkdocs-static-i18n==1.2.* pymdown-extensions
   mkdocs build --strict
   deactivate
   # Deve compilare senza errori

7. Abilita GitHub Pages:
   # Crea branch gh-pages se non esiste
   git ls-remote --exit-code origin gh-pages || (
     git checkout --orphan gh-pages &&
     git rm -rf . &&
     echo "# hfs docs" > index.html &&
     git add index.html &&
     git commit -m "chore: init gh-pages branch" &&
     git push origin gh-pages &&
     git checkout main
   )
   # Abilita Pages via gh CLI
   gh api repos/disoardi/hfs/pages -X POST \
     -f source.branch=gh-pages -f source.path=/ 2>/dev/null || true
   # Primo deploy
   source ~/venvs/mkdocs/bin/activate
   mkdocs gh-deploy --force
   deactivate

8. Verifica CI:
   git add .github/workflows/
   git commit -m "ci: add CI and docs workflows" || true
   git push origin main
   # Controlla: gh run list --limit 3

Al termine, riporta:
- URL GitHub Pages (https://disoardi.github.io/hfs)
- Stato Docker cluster (namenode healthy: sì/no)
- Esito cargo build --workspace
- Lista fixture caricate in HDFS
```

---

## Prompt Giorno 1 — WebHDFS backend + ls/stat/du

```
Stiamo sviluppando `hfs`. Leggi CLAUDE.md prima di fare qualsiasi cosa.

Oggi: Giorno 1 — implementare il WebHDFS backend e i comandi ls/stat/du.

Prerequisito: assicurati che il cluster Docker sia avviato:
  docker compose -f docker/docker-compose.test.yml ps
  # Se non è up: docker compose -f docker/docker-compose.test.yml up -d

Obiettivi:

1. Creare hfs-core/src/webhdfs.rs — WebHdfsClient che implementa HdfsClient trait
   - Usa ureq con feature "tls" (rustls, NON native-tls)
   - list(): GET /webhdfs/v1{path}?op=LISTSTATUS
   - stat(): GET /webhdfs/v1{path}?op=GETFILESTATUS
   - content_summary(): GET /webhdfs/v1{path}?op=GETCONTENTSUMMARY
   - read_range(): GET /webhdfs/v1{path}?op=OPEN + header Range: bytes=N-M
   - health(): JMX endpoint ?qry=Hadoop:service=NameNode,name=FSNamesystemState
   - file_size(): stat().length
   - Errori HTTP → HfsError (404 → NotFound, 403 → Permission, rete → Connection)
   - Zero unwrap() — usa sempre ? o map_err

2. Completare hfs-core/src/config.rs
   - Parsing core-site.xml con quick-xml (NON libxml2)
   - Legge fs.defaultFS → namenode_uri
   - Legge hadoop.security.authentication → simple | kerberos
   - Priority chain: CLI flag → env HFS_NAMENODE → ~/.hfs/config.toml → HADOOP_CONF_DIR → /etc/hadoop/conf → localhost:9870
   - Test: usa tests/fixtures/core-site-minimal.xml (crealo se manca)

3. Implementare comandi ls/stat/du in hfs/src/main.rs
   - ls: tabella comfy-table con Permission/Repl/Owner/Group/Size/Modified/Path
   - ls --json: serde_json::to_string_pretty
   - stat: info dettagliata singolo path
   - du: spazio usato, numero file, formato human-readable (KB/MB/GB)

4. Test OBBLIGATORI:
   a) Unit test con mockito:
      - mock GET /webhdfs/v1/?op=LISTSTATUS → fixture JSON
      - verifica che WebHdfsClient::list() ritorni Vec<FileStatus> corretto
      - verifica che stat() su path non esistente ritorni HfsError::NotFound
   b) Integration test su Docker (marcati #[ignore]):
      - hfs ls / → deve ritornare almeno 1 entry
      - hfs ls /test-data/parquet/ → deve trovare small.parquet
      - hfs health → NumLiveDataNodes >= 1

5. Aggiorna documentazione:
   - docs/comandi.md: sezione "ls", "stat", "du" con esempi di output
   - docs/en/commands.md: stesso contenuto in inglese
   - Verifica: mkdocs build --strict

Verifica finale:
  cargo fmt --check
  cargo clippy --workspace -- -D warnings
  cargo test --workspace
  HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
  HFS_NAMENODE=http://localhost:9870 cargo run --bin hfs -- ls /test-data/
```

---

## Prompt Giorno 2 — RPC backend + auto-detection

```
Stiamo sviluppando `hfs`. Leggi CLAUDE.md prima di fare qualsiasi cosa.

Oggi: Giorno 2 — RPC backend via hdfs-native e auto-detection del backend.

Prerequisito: Docker cluster up, test Giorno 1 verdi.

Obiettivi:

1. Creare hfs-core/src/rpc.rs — RpcClient che implementa HdfsClient
   - Wrappa hdfs_native::HdfsClient
   - Implementa list(), stat(), content_summary(), read_range(), health()
   - Gestisci errori hdfs-native → HfsError con messaggi descrittivi
   - Se hdfs-native manca di qualche operazione: usa TODO! con descrizione
     e per quella specifica op fai fallback a WebHdfsClient (passando il webhdfs_url
     da HdfsConfig)

2. Creare hfs-core/src/builder.rs — HdfsClientBuilder
   - build() → Box<dyn HdfsClient>
   - Auto-detection: prova TCP connect su porta 8020 con timeout 2s
   - Se connessione rifiutata o timeout → WebHdfsClient
   - Se --backend rpc → RpcClient diretto (errore se connessione fallisce)
   - Se --backend webhdfs → WebHdfsClient diretto
   - --show-backend → stampa su stderr "[backend: RPC]" o "[backend: WebHDFS]"

3. Aggiornare hfs/src/main.rs
   - Tutti i comandi usano HdfsClientBuilder invece di WebHdfsClient diretto
   - --show-backend flag: stampa quale backend è attivo

4. Test OBBLIGATORI:
   a) Unit test auto-detection: mock TCP (porta chiusa) → verifica che selezioni WebHDFS
   b) Integration test su Docker #[ignore]:
      - con backend rpc su porta 8020 → verifica [backend: RPC]
      - con backend webhdfs esplicito → verifica [backend: WebHDFS]
      - senza backend → verifica auto-detection (dipende da cosa espone il Docker)
   c) Test failover: avvia con porta RPC bloccata → deve usare WebHDFS automaticamente

5. Aggiorna documentazione:
   - docs/configurazione.md: sezione --backend, auto-detection
   - docs/en/configuration.md: stesso in inglese
   - mkdocs build --strict

Verifica finale:
  cargo fmt --check
  cargo clippy --workspace -- -D warnings
  cargo test --workspace
  HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
  HFS_NAMENODE=http://localhost:9870 cargo run --bin hfs -- --show-backend ls /
```

---

## Prompt Giorno 3 — Schema Parquet (footer reading)

```
Stiamo sviluppando `hfs`. Leggi CLAUDE.md prima di fare qualsiasi cosa.

Oggi: Giorno 3 — lettura schema Parquet dal solo footer (O(KB) non O(MB)).

Prerequisito: Docker cluster up, test Giorno 1-2 verdi.

Obiettivi:

1. Completare hfs-schema/src/lib.rs
   - SeekableReader trait: read_range(offset, len) → Vec<u8>
   - HdfsRangeReader: implementa SeekableReader wrappando HdfsClient::read_range()
   - LocalFileReader: implementa SeekableReader da file locale (per test)
   - detect_format(): riconosce Parquet (magic PAR1), Avro (Obj\x01), ORC (ORC)

2. Completare hfs-schema/src/parquet.rs — ParquetInspector::inspect()
   Algoritmo ESATTO (2 richieste HTTP, zero dati):
   a) read_range(file_size - 8, 8) → ultimi 8 byte
      - verifica magic bytes: ultimi 4 = b"PAR1"
      - footer_len = u32::from_le_bytes(bytes[0..4]) as u64
   b) read_range(file_size - 8 - footer_len, footer_len) → footer bytes
   c) parquet::file::metadata::ParquetMetaData::decode(&footer_bytes)
   d) Converti Arrow schema → Schema unificato hfs (FieldType enum)
   e) Estrai column stats (min/max/null_count) da ogni RowGroup
   f) Ritorna ParquetMeta { schema, row_groups, total_rows, file_size }

3. Implementare comandi schema/stats/rowcount in hfs/src/main.rs
   - hfs schema /path/file.parquet → albero schema con tipi e nullable
   - hfs schema /path/ → schema primo file + "N altri file con schema identico / M con divergenze"
   - hfs stats /path/file.parquet → tabella colonne con min/max/null_count/distinct_count
   - hfs rowcount /path/ → somma row_count da tutti i Parquet

4. Test OBBLIGATORI:
   a) Scarica fixtures se non presenti:
      curl -L https://github.com/apache/parquet-testing/raw/master/data/alltypes_plain.parquet \
        -o tests/fixtures/small.parquet
      curl -L https://github.com/apache/parquet-testing/raw/master/data/nested_structs.rust.parquet \
        -o tests/fixtures/nested.parquet
   b) Unit test LocalFileReader su tests/fixtures/small.parquet:
      - verifica schema corretto (tipi corrispondono all'atteso)
      - verifica row_count atteso
      - CRITICO: conta le chiamate a read_range() → devono essere ESATTAMENTE 2
   c) Unit test mockito HdfsRangeReader:
      - mock read_range su file 100MB simulato → verifica 2 chiamate HTTP
      - verifica offset corretto: (100*1024*1024 - 8) per la prima chiamata
   d) Integration test su Docker #[ignore]:
      - hfs schema /test-data/parquet/small.parquet → schema non vuoto
      - hfs rowcount /test-data/parquet/ → > 0

5. Aggiorna documentazione:
   - docs/comandi.md: sezione "schema", "stats", "rowcount" con output di esempio
   - docs/en/commands.md: stesso in inglese
   - mkdocs build --strict

Verifica finale:
  cargo fmt --check
  cargo clippy --workspace -- -D warnings
  cargo test --workspace
  HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
  HFS_NAMENODE=http://localhost:9870 cargo run --bin hfs -- schema /test-data/parquet/small.parquet
```

---

## Prompt Giorno 4 — Blocks / replicas / health + config

```
Stiamo sviluppando `hfs`. Leggi CLAUDE.md prima di fare qualsiasi cosa.

Oggi: Giorno 4 — block inspection, cluster health, config auto-detect.

Prerequisito: Docker cluster up, test Giorno 1-3 verdi.

Obiettivi:

1. Implementare hfs blocks in hfs/src/main.rs + hfs-core:
   - WebHDFS: GET ?op=GETFILEBLOCKLOCATIONS → lista blocchi con DataNode e racks
   - Output: tabella blocco/offset/lunghezza/DataNodes
   - --json: JSON strutturato

2. Implementare hfs replicas /path --min-replication N:
   - Usa WebHDFS content_summary + per-file replication factor
   - Elenca file con replication < N
   - Default N = dfs.replication.min da core-site.xml o 1

3. Completare config.rs — parsing core-site.xml completo:
   - quick-xml per leggere property name/value
   - Mappa: fs.defaultFS, dfs.namenode.rpc-address, hadoop.security.authentication,
     dfs.replication, dfs.replication.min, dfs.webhdfs.enabled
   - Test con tests/fixtures/core-site-minimal.xml
   - Test con tests/fixtures/core-site-kerb.xml (ha hadoop.security.authentication=kerberos)

4. Implementare hfs health:
   - JMX NameNode metrics: NumLiveDataNodes, NumDeadDataNodes, NumStaleDataNodes,
     CapacityUsed, CapacityTotal, UnderReplicatedBlocks, CorruptBlocks, HAState
   - Output: dashboard testuale colorata (verde/giallo/rosso in base a soglie)
   - --json: ClusterHealth struct serializzata

5. Implementare hfs small-files /path --threshold 128M:
   - Content summary ricorsivo
   - avg_file_size = space_consumed / file_count per ogni subdirectory
   - Stampa directory con avg < threshold, ordinate per file_count desc
   - Crea fixture in Docker: docker exec hfs-test-client /load-fixtures.sh
     (lo script crea già 20 file piccoli in /test-data/small-files/)

6. Test OBBLIGATORI:
   a) Unit test config.rs: parsing core-site-minimal.xml → valori corretti
   b) Unit test health con mockito: JMX response → ClusterHealth corretto
   c) Integration test Docker #[ignore]:
      - hfs health → NumLiveDataNodes = 2 (ci sono 2 datanode nel compose)
      - hfs small-files /test-data/small-files/ → deve trovare molti file piccoli
      - hfs blocks /test-data/parquet/small.parquet → almeno 1 blocco

7. Aggiorna documentazione:
   - docs/comandi.md: sezione "blocks", "replicas", "health", "small-files"
   - docs/configurazione.md: core-site.xml auto-detection
   - docs/en/: aggiorna entrambe
   - mkdocs build --strict

Verifica finale:
  cargo fmt --check
  cargo clippy --workspace -- -D warnings
  cargo test --workspace
  HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
  HFS_NAMENODE=http://localhost:9870 cargo run --bin hfs -- health
```

---

## Prompt Giorno 5 — Schema drift vs Hive + packaging + release v0.1.0

```
Stiamo sviluppando `hfs`. Leggi CLAUDE.md e DEV_FLOW.md prima di fare qualsiasi cosa.

Oggi: Giorno 5 — Hive Metastore, drift detection, release MVP.

Prerequisito: Docker cluster up (include HMS), test Giorno 1-4 verdi.

Obiettivi:

1. Completare hfs-schema/src/hive.rs — HiveMetastoreClient
   - Prova prima HMS HTTP API: GET http://hive-metastore:8080/api/v1/database/{db}/table/{table}
   - Se HTTP fallisce o ritorna 404: Thrift TCP porta 9083
     (Per Thrift usa un raw TCP socket con serializzazione Binary — la spec è semplice
     per GetTable; in alternativa genera i binding da thrift-rs se disponibile)
   - Converti FieldSchema[] → Schema unificato hfs
   - Test con mock HMS HTTP server via mockito
   - Integration test su Docker #[ignore]: HMS su localhost:9083 / localhost:8080

2. Creare tabella di test in Hive per i test di drift:
   docker exec hfs-test-client bash -c "
     beeline -u 'jdbc:hive2://hive-metastore:10000' -e '
       CREATE TABLE IF NOT EXISTS test_db.transactions (
         tx_id BIGINT,
         amount DECIMAL(18,2),
         ts TIMESTAMP
       ) STORED AS PARQUET
       LOCATION \"/test-data/parquet/\";
     '
   "
   # Se Hive non è disponibile nel compose, usa HMS HTTP mock via mockito per il test

3. Implementare hfs drift /path --against hive://db.table:
   - Leggi schema di tutti i Parquet nel path (ParquetInspector)
   - Leggi schema tabella Hive (HiveMetastoreClient)
   - SchemaDiff::compare(parquet_schema, hive_schema) per ogni file
   - Output colorato: breaking change = rosso, aggiuntivo = giallo, ok = verde
   - --json: DriftReport serializzato

4. Implementare hfs schema /path --against hive://db.table:
   - Schema del file/path con differenze evidenziate vs Hive

5. Packaging:
   - Crea tuxbox.toml in root:
     ```toml
     [tool]
     name = "hfs"
     version = "0.1.0"
     description = "HDFS CLI senza JVM"
     entrypoint = "hfs"
     isolation = "none"
     ```
   - Build statico: cargo build --release --target x86_64-unknown-linux-musl
   - Verifica dimensione: ls -lh target/x86_64-unknown-linux-musl/release/hfs
     Target: < 15MB
   - Verifica zero dipendenze: ldd target/.../hfs → "not a dynamic executable"

6. Documentazione finale:
   - Completa tutte le sezioni di docs/comandi.md con esempi reali di output
   - Completa docs/en/commands.md
   - mkdocs build --strict
   - Verifica che GitHub Pages sia aggiornato: mkdocs gh-deploy --force

7. Release v0.1.0:
   - Aggiorna version in hfs-core, hfs-schema, hfs Cargo.toml
   - Verifica: grep -r "patch.crates-io" Cargo.toml → deve essere commentato
   - cargo test --workspace
   - git tag -a v0.1.0 -m "v0.1.0 — MVP: ls, du, stat, health, schema Parquet, drift"
   - git push origin main --tags
   - gh release create v0.1.0 target/.../hfs#hfs-linux-x86_64 --generate-notes

8. Upstream candidates — apri issue (non PR ancora):
   - Stai leggendo DEV_FLOW.md per il processo
   - Per ogni gap trovato in hdfs-native durante il Giorno 1-5:
     gh issue create --repo datafusion-contrib/datafusion-hdfs-native \
       --title "..." --body "..."
   - Solo issue, non PR — prima costruisci engagement con la community

Verifica finale:
  cargo fmt --check
  cargo clippy --workspace -- -D warnings
  cargo test --workspace
  HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
  ldd target/x86_64-unknown-linux-musl/release/hfs
  ls -lh target/x86_64-unknown-linux-musl/release/hfs
```

---

## Prompt sessione Kerberos (dopo v0.1.0)

```
Stiamo aggiungendo il supporto Kerberos completo a hfs. Leggi CLAUDE.md e DEV_FLOW.md.

Prerequisito: v0.1.0 rilasciata, test tutti verdi.

Oggi: implementare autenticazione Kerberos per RPC backend (SASL).

1. Avvia cluster Kerberos Docker:
   docker compose -f docker/docker-compose.kerb.yml up -d
   docker compose -f docker/docker-compose.kerb.yml exec freeipa /scripts/wait-for-ipa.sh
   docker compose -f docker/docker-compose.kerb.yml exec freeipa /scripts/setup-principals.sh

2. Verifica che WebHDFS SPNEGO funzioni già senza feature flag:
   docker compose -f docker/docker-compose.kerb.yml exec hdfs-client \
     kinit -kt /keytabs/hfs.keytab hfs/hdfs-client.hfs.test@HFS.TEST
   HFS_NAMENODE=http://namenode-kerb:9870 \
     cargo run --bin hfs -- --backend webhdfs ls /

3. Implementa RPC Kerberos (feature kerberos):
   - In hfs-core/src/rpc.rs: se feature kerberos attivo e config.kerberos = true
     → usa libgssapi-sys per SASL handshake prima del primo RPC
   - Feature flag: #[cfg(feature = "kerberos")]
   - Se feature non attivo e cluster richiede Kerberos: HfsError::Auth con messaggio
     "Kerberos required — build with --features kerberos"

4. Test:
   docker compose -f docker/docker-compose.kerb.yml exec hdfs-client bash -c "
     kinit -kt /keytabs/hfs.keytab hfs/hdfs-client.hfs.test@HFS.TEST
     HFS_NAMENODE=hdfs://namenode-kerb:8020 HFS_KERBEROS=true \
     cargo test --features kerberos --workspace -- --include-ignored
   "

5. Aggiorna docs/kerberos.md e docs/en/kerberos.md con:
   - Prerequisiti (libgssapi-dev, kinit)
   - Build con --features kerberos
   - Esempi di utilizzo su cluster Kerberos
   - Nota su WebHDFS SPNEGO vs RPC SASL

6. mkdocs build --strict
   mkdocs gh-deploy --force
```

---

## Prompt sessione upstream (patch a datafusion-hdfs-native)

```
Oggi lavoriamo su una contribuzione upstream a datafusion-hdfs-native.
Leggi CLAUDE.md e DEV_FLOW.md prima di tutto — in particolare la sezione "Ciclo upstream".

Il problema da risolvere in hdfs-native è:
[INSERISCI QUI: descrizione precisa del bug o feature mancante, es.
 "read_range() non funziona correttamente quando offset > 2^32 (file > 4GB)"]

Procedura:
1. Verifica di essere su un branch feat/upstream-* (MAI su main)
   git checkout -b feat/upstream-[nome-breve]

2. Attiva [patch.crates-io] in Cargo.toml workspace
   (decommentare il blocco hdfs-native = { path = "../datafusion-hdfs-native" })

3. Implementa la fix nel clone locale di datafusion-hdfs-native
   cd ~/Progetti/datafusion-hdfs-native
   git checkout -b fix/[nome-breve]

4. Scrivi test che dimostrano il problema E la soluzione

5. Testa via hfs su Docker:
   docker compose -f docker/docker-compose.test.yml up -d
   cargo build && HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored

6. Prepara commit pulito:
   - Nessun riferimento a hfs, Davide, o strumenti AI
   - Solo la modifica minimale necessaria
   - Commit message in inglese, descrittivo, formato upstream

7. Segui DEV_FLOW.md sezione "Ciclo upstream" per aprire la PR

Nota: i commenti nel codice devono essere in inglese.
Il commit message deve essere in inglese e seguire il formato dei commit esistenti nel repo upstream.
```

---

## Note generali per tutte le sessioni

- Prima di ogni commit: `cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace`
- Prima di ogni merge in main: integration test su Docker + `mkdocs build --strict`
- Prima di ogni release: `grep -r "patch.crates-io" Cargo.toml` → deve essere commentato
- Log di debug: `RUST_LOG=debug cargo run --bin hfs -- <comando>`
- Coverage: `cargo tarpaulin --workspace --out Html`
- Interazione con Davide: sempre in italiano
