# DEV_FLOW — Flusso di sviluppo hfs

---

## Setup iniziale (una volta sola)

### 1. Repository GitHub

```bash
# Crea repo PRIVATO disoardi/hfs
gh repo create disoardi/hfs --private \
  --description "HDFS CLI tool — no JVM required" \
  --confirm

# Push iniziale
cd ~/Progetti/hfs
git remote add origin git@github.com:disoardi/hfs.git
git push -u origin main

# Fork datafusion-hdfs-native e aggiunge remote upstream
gh repo fork datafusion-contrib/datafusion-hdfs-native --clone=false
cd ~/Progetti
git clone git@github.com:disoardi/datafusion-hdfs-native.git
cd datafusion-hdfs-native
git remote add upstream git@github.com:datafusion-contrib/datafusion-hdfs-native.git
git fetch upstream
```

Struttura risultante:
```
~/Progetti/
  hfs/                          ← repo privato principale
  datafusion-hdfs-native/       ← fork pubblico (per ciclo upstream)
```

### 2. Toolchain Rust

```bash
# Stable toolchain + target musl
rustup toolchain install stable
rustup target add x86_64-unknown-linux-musl

# Su Ubuntu/Debian: strumenti per build musl
sudo apt install musl-tools

# Tool di sviluppo
cargo install cargo-tarpaulin   # coverage
cargo install cargo-outdated    # check dipendenze aggiornabili
cargo install cargo-audit       # security audit
```

### 3. Ambiente Docker per test

```bash
cd ~/Progetti/hfs

# Avvia cluster HDFS locale (namenode + 2 datanode + HMS)
docker compose -f docker/docker-compose.test.yml up -d

# Aspetta che namenode sia healthy
docker compose -f docker/docker-compose.test.yml ps

# Carica fixture di test
docker exec hfs-test-client /load-fixtures.sh

# Verifica che HDFS risponda
curl -s "http://localhost:9870/jmx?qry=Hadoop:service=NameNode,name=NameNodeStatus" \
  | python3 -m json.tool | head -20

# Stop quando non serve
docker compose -f docker/docker-compose.test.yml down
```

### 4. Documentazione locale

```bash
# Crea venv Python per mkdocs (non inquina il sistema)
python3 -m venv ~/venvs/mkdocs
source ~/venvs/mkdocs/bin/activate
pip install mkdocs-material==9.5.* mkdocs-static-i18n==1.2.* pymdown-extensions

# Avvia server locale
mkdocs serve
# Apri http://localhost:8000

# Deactivate quando finito
deactivate
```

### 5. GitHub Pages — abilitazione iniziale

```bash
# Crea branch gh-pages vuoto (GitHub Pages lo usa)
git checkout --orphan gh-pages
git rm -rf .
echo "# Placeholder" > index.html
git add index.html
git commit -m "chore: init gh-pages branch"
git push origin gh-pages
git checkout main

# Su GitHub: Settings → Pages → Source: "Deploy from branch" → gh-pages / root
# Oppure via gh CLI:
gh api repos/disoardi/hfs/pages -X POST -f source.branch=gh-pages -f source.path=/

# Primo deploy manuale
source ~/venvs/mkdocs/bin/activate
mkdocs gh-deploy --force
deactivate

# Dopo il setup, il deploy è automatico via .github/workflows/docs.yml
```

---

## Branch Strategy — repo `hfs`

```
main
  └── Sempre compilabile con hdfs-native da crates.io
  └── [patch.crates-io] SEMPRE commentato
  └── Tutti i test passano: cargo test --workspace
  └── Tag di release: v0.1.0, v0.2.0 ...

feat/<nome>
  └── Nuove feature di hfs
  └── Merge in main via PR quando: test verde, clippy pulito, docs aggiornate

fix/<nome>
  └── Bug fix interni a hfs

feat/upstream-<nome>
  └── Branch per sviluppare qualcosa da proporre upstream
  └── [patch.crates-io] ATTIVO solo qui
  └── NON mergiare in main con il patch attivo
```

---

## Ciclo di sviluppo normale (feature di hfs)

```bash
git checkout -b feat/ls-command

# Sviluppa — ricorda: commenti in inglese, niente unwrap()
# ...

# Verifica pre-commit
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

# Integration test su Docker
docker compose -f docker/docker-compose.test.yml up -d
HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
docker compose -f docker/docker-compose.test.yml down

# Aggiorna documentazione se il comportamento è cambiato
# Edita docs/comandi.md e docs/en/commands.md
source ~/venvs/mkdocs/bin/activate
mkdocs build --strict    # verifica che la doc compili senza warning
deactivate

git add -p
git commit -m "feat: implement hfs ls with human-readable sizes"
git push origin feat/ls-command
gh pr create --title "feat: implement ls command" --body "Closes #N"
```

---

## Ciclo upstream — contribuire a datafusion-hdfs-native

Scenario: durante sviluppo di `hfs-core` trovi che `hdfs-native` ha un bug
o manca di una funzione (es. `read_range` con offset arbitrario).

```bash
# 1. Crea branch di lavoro in hfs
git checkout -b feat/upstream-pread-support

# 2. Attiva [patch.crates-io] in Cargo.toml workspace
# Decommentare il blocco:
# [patch.crates-io]
# hdfs-native = { path = "../datafusion-hdfs-native" }

# 3. Implementa nel fork locale
cd ~/Progetti/datafusion-hdfs-native
git checkout -b fix/pread-offset-support
# ... implementa la fix ...
cargo test
cargo clippy -- -D warnings

# 4. Testa via hfs con il patch attivo
cd ~/Progetti/hfs
cargo build    # usa il fork locale via [patch.crates-io]
cargo test -p hfs-core

# Test su cluster Docker
docker compose -f docker/docker-compose.test.yml up -d
HFS_NAMENODE=http://localhost:9870 cargo run --bin hfs -- schema /test-data/parquet/small.parquet
docker compose -f docker/docker-compose.test.yml down

# 5. Fix validato — prepara commit pulito per upstream
cd ~/Progetti/datafusion-hdfs-native
# Verifica che i commit siano puliti:
# - nessun riferimento a "hfs", "Davide", strumenti AI
# - solo la modifica minimale necessaria
# - test aggiunti per la fix
git log --oneline -5

# Sync con upstream per evitare conflitti
git fetch upstream
git rebase upstream/main

git push origin fix/pread-offset-support

# 6. Apri PR su GitHub
# Da: disoardi/datafusion-hdfs-native:fix/pread-offset-support
# A:  datafusion-contrib/datafusion-hdfs-native:main
gh pr create \
  --repo datafusion-contrib/datafusion-hdfs-native \
  --head disoardi:fix/pread-offset-support \
  --title "Add pread support for arbitrary offset reads" \
  --body "$(cat <<'EOF'
## Problem

`HdfsClient::read` always reads from the beginning of the file. This makes it
impossible to implement efficient footer reading for columnar formats like Parquet,
which store metadata at the end of the file.

## Solution

Add a `read_range(path, offset, length)` method to `HdfsClient` that maps to
`LocatedBlocks` + block-level reads for the RPC backend, and to WebHDFS
Range requests for the HTTP backend.

## Tests

- Unit test: `test_read_range_offset` with a mock HDFS server
- Integration test: reads last 8 bytes of a 100MB file correctly
EOF
)"
# NON menzionare hfs o AI
```

```bash
# 7. Dopo merge upstream — torna a hfs
cd ~/Progetti/hfs

# Aggiorna versione in Cargo.toml workspace
# hdfs-native = "0.9.X"  (nuova versione con il fix)

# Commenta il [patch.crates-io]
# [patch.crates-io]
# hdfs-native = { path = "../datafusion-hdfs-native" }  # COMMENTATO

cargo build    # verifica che compili con la versione pubblica
cargo test --workspace

git add Cargo.toml Cargo.lock
git commit -m "chore: update hdfs-native to 0.9.X (includes read_range fix)"
git checkout main
git merge feat/upstream-pread-support --no-ff
git push origin main
git branch -d feat/upstream-pread-support
```

---

## Sincronizzare il fork con upstream

```bash
cd ~/Progetti/datafusion-hdfs-native
git fetch upstream
git checkout main
git rebase upstream/main
git push origin main
```

---

## Test Kerberos (cluster con FreeIPA)

```bash
cd ~/Progetti/hfs

# Avvia cluster Kerberos (HDFS + FreeIPA KDC)
docker compose -f docker/docker-compose.kerb.yml up -d

# FreeIPA impiega 3-5 minuti al primo avvio — aspetta
docker compose -f docker/docker-compose.kerb.yml exec freeipa /scripts/wait-for-ipa.sh

# Crea principal e keytab (solo al primo avvio)
docker compose -f docker/docker-compose.kerb.yml exec freeipa /scripts/setup-principals.sh

# Ottieni ticket Kerberos dal container client
docker compose -f docker/docker-compose.kerb.yml exec hdfs-client \
  kinit -kt /keytabs/hfs.keytab hfs/hdfs-client.hfs.test@HFS.TEST

# Test con Kerberos (--include-ignored esegue anche i test #[ignore])
docker compose -f docker/docker-compose.kerb.yml exec hdfs-client bash -c '
  cd /workspace && \
  HFS_NAMENODE=hdfs://namenode-kerb:8020 \
  HFS_KERBEROS=true \
  cargo test --features kerberos --workspace -- --include-ignored
'

# Teardown
docker compose -f docker/docker-compose.kerb.yml down -v
```

---

## Aggiornamento documentazione

Ogni modifica al comportamento osservabile di `hfs` (nuovi comandi, flag, output)
**deve** aggiornare la documentazione corrispondente.

```bash
# Regola: se cambi codice che tocca l'utente → aggiorna docs/ nello stesso PR

# Verifica che la doc compili senza errori
source ~/venvs/mkdocs/bin/activate
mkdocs build --strict
deactivate

# Preview locale durante sviluppo
source ~/venvs/mkdocs/bin/activate
mkdocs serve &
# Apri http://localhost:8000, naviga la pagina modificata
deactivate

# Deploy manuale (opzionale — la CI fa il deploy automatico su push main)
source ~/venvs/mkdocs/bin/activate
mkdocs gh-deploy --force
deactivate
```

### Cosa aggiornare per tipo di modifica

| Tipo di modifica | File da aggiornare |
|------------------|--------------------|
| Nuovo comando | `docs/comandi.md` + `docs/en/commands.md` |
| Nuovo flag globale | `docs/configurazione.md` + EN |
| Nuova variabile env / config | `docs/configurazione.md` + EN |
| Cambia output di un comando | `docs/comandi.md` (esempi di output) |
| Kerberos-related | `docs/kerberos.md` + EN |
| Cambia procedura di build | `docs/installazione.md` + EN |

---

## Release di hfs

```bash
cd ~/Progetti/hfs
git checkout main

# 1. Pre-release checks
cargo test --workspace
cargo clippy --workspace -- -D warnings
grep -q "^hdfs-native = { path" Cargo.toml && echo "⚠️  PATCH ATTIVA — rimuovila prima della release!" || echo "✅ No patch attiva"
mkdocs build --strict

# 2. Aggiorna versione
# In hfs-core/Cargo.toml, hfs-schema/Cargo.toml, hfs/Cargo.toml: version = "0.X.Y"

# 3. Build binari
cargo build --release --target x86_64-unknown-linux-musl
# Su macOS o con cross:
# cross build --release --target aarch64-apple-darwin

# 4. Verifica dimensione
ls -lh target/x86_64-unknown-linux-musl/release/hfs
# Target: < 15MB

# 5. Commit e tag
git add hfs-core/Cargo.toml hfs-schema/Cargo.toml hfs/Cargo.toml
git commit -m "chore: release v0.X.Y"
git tag -a v0.X.Y -m "v0.X.Y — [descrizione breve]"
git push origin main --tags

# 6. GitHub Release con binari
gh release create v0.X.Y \
  target/x86_64-unknown-linux-musl/release/hfs#hfs-linux-x86_64 \
  --title "hfs v0.X.Y" \
  --generate-notes

# 7. Deploy documentazione aggiornata
source ~/venvs/mkdocs/bin/activate
mkdocs gh-deploy --force
deactivate
```

---

## Comandi di sviluppo quotidiani

```bash
# Build veloce
cargo build -p hfs

# Test selettivi
cargo test -p hfs-core
cargo test -p hfs-schema
cargo test --workspace -- --nocapture   # con output
cargo test --workspace test_webhdfs     # test specifico per nome

# Coverage
cargo tarpaulin --workspace --out Html
open tarpaulin-report.html

# Debug con log
RUST_LOG=debug cargo run --bin hfs -- ls /

# Integration test rapido su Docker
docker compose -f docker/docker-compose.test.yml up -d
HFS_NAMENODE=http://localhost:9870 cargo run --bin hfs -- ls /
docker compose -f docker/docker-compose.test.yml down

# Analisi dipendenze
cargo outdated
cargo audit
cargo tree
```

---

## Checklist prima di ogni PR upstream a datafusion-hdfs-native

- [ ] I commit sono atomici e hanno messaggi in inglese descrittivi
- [ ] Nessun riferimento a "hfs", "Davide", strumenti AI nel codice/commit/PR
- [ ] Test aggiunti per la fix/feature
- [ ] `cargo test` passa sul fork
- [ ] `cargo clippy -- -D warnings` senza errori
- [ ] `cargo fmt --check` pulito
- [ ] Codice segue lo stile del repo originale
- [ ] Letto CONTRIBUTING.md del repo upstream (se esiste)
- [ ] Verificato che non ci siano issue aperti simili già in corso
- [ ] Pronto a rispondere domande tecniche in inglese
