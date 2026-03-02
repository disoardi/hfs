# DEV_FLOW — Flusso di sviluppo hfs

---

## Setup iniziale (una volta sola)

```bash
# 1. Crea i repo GitHub
# Su github.com: nuovo repo "hfs" (PRIVATO), "datafusion-hdfs-native" (PUBLIC, fork)

# 2. Clona i repo
cd ~/Progetti
git clone git@github.com:disoardi/hfs.git                         # già esistente localmente
git clone git@github.com:disoardi/datafusion-hdfs-native.git       # fork personale

# Aggiungi upstream al fork per ricevere aggiornamenti
cd datafusion-hdfs-native
git remote add upstream https://github.com/datafusion-contrib/datafusion-hdfs-native.git
git fetch upstream

# 3. Struttura risultante
~/Progetti/
  hfs/                          ← repo privato principale
  datafusion-hdfs-native/       ← fork pubblico, accanto a hfs (per [patch.crates-io])
```

---

## Branch Strategy — repo `hfs`

```
main
  └── Sempre compilabile con hdfs-native da crates.io
  └── Nessun [patch.crates-io] attivo
  └── Tag di release: v0.1.0, v0.2.0 ...

feat/<nome>
  └── Nuove feature di hfs
  └── Merge in main via PR (anche se sei solo tu)

fix/hfs-<nome>
  └── Bug fix interni a hfs, non dipendenti da hdfs-native

feat/upstream-<nome>
  └── Branch dove sviluppi qualcosa che poi vuoi proporre upstream
  └── Usa [patch.crates-io] qui
  └── NON mergiare in main finché il patch è attivo
  └── Dopo merge upstream: rimuovi patch, aggiorna versione, merge in main
```

---

## Ciclo di sviluppo normale (feature di hfs)

```bash
git checkout -b feat/ls-command
# ... sviluppa ...
cargo test
cargo clippy -- -D warnings
git add -p
git commit -m "feat: implement hfs ls with human-readable sizes"
git push origin feat/ls-command
# Su GitHub: apri PR, review, merge in main
```

---

## Ciclo upstream — contribuire a datafusion-hdfs-native

Scenario: durante sviluppo di `hfs-core` trovi che `hdfs-native` non supporta
`pread` con offset arbitrario (necessario per leggere il footer Parquet).

```bash
# 1. Crea branch di lavoro in hfs
git checkout -b feat/upstream-pread-support

# 2. Attiva [patch.crates-io] in Cargo.toml workspace
# Decommentare:
# [patch.crates-io]
# hdfs-native = { path = "../datafusion-hdfs-native" }

# 3. Sviluppa il fix in datafusion-hdfs-native
cd ~/Progetti/datafusion-hdfs-native
git checkout -b fix/pread-offset-support
# ... implementa ...
cargo test
```

```bash
# 4. Testa il fix via hfs (con il patch attivo)
cd ~/Progetti/hfs
cargo build    # usa il fork locale via patch
cargo test -p hfs-core
# Test su cluster reale
export HFS_NAMENODE=hdfs://namenode.corp.com:8020
cargo run --bin hfs -- schema /path/big_file.parquet
```

```bash
# 5. Fix validato → prepara PR upstream
cd ~/Progetti/datafusion-hdfs-native

# Assicurati che il commit sia pulito:
# - nessun riferimento a "hfs" nel codice o nei commit
# - solo la modifica minimale necessaria
# - test aggiunti per la nuova funzionalità
git log --oneline -5  # verifica che i commit siano sensati

# Sync con upstream per evitare conflitti
git fetch upstream
git rebase upstream/main

git push origin fix/pread-offset-support
```

```bash
# 6. Apri PR su GitHub
# Da: disoardi/datafusion-hdfs-native:fix/pread-offset-support
# A:  datafusion-contrib/datafusion-hdfs-native:main
#
# Titolo: "Add pread support for arbitrary offset reads"
# Description: spiega il problema tecnico, la soluzione, i test
# NON menzionare hfs o AI
# Sii pronto a rispondere a domande tecniche sui maintainer
```

```bash
# 7. Dopo merge upstream — torna a hfs
cd ~/Progetti/hfs

# Aggiorna la dipendenza a crates.io
# In Cargo.toml workspace: hdfs-native = "0.9.X"  (nuova versione con il fix)

# Commenta il [patch.crates-io]
# [patch.crates-io]
# hdfs-native = { path = "../datafusion-hdfs-native" }  # COMMENTATO

cargo build   # verifica che compili con la versione pubblica
cargo test

git add Cargo.toml Cargo.lock
git commit -m "chore: update hdfs-native to 0.9.X (includes pread fix)"
git checkout main
git merge feat/upstream-pread-support
git push origin main
git branch -d feat/upstream-pread-support
```

---

## Sincronizzare il fork con upstream

```bash
cd ~/Progetti/datafusion-hdfs-native
git fetch upstream
git checkout main
git rebase upstream/main   # o merge, dipende dalla preference
git push origin main       # aggiorna il fork su GitHub
```

---

## Release di hfs

```bash
cd ~/Progetti/hfs
git checkout main

# Verifica che tutto sia a posto
cargo test --workspace
cargo clippy --workspace -- -D warnings
grep -r "patch.crates-io" Cargo.toml && echo "ATTENZIONE: patch attiva!" || echo "OK"

# Aggiorna versione in tutti i Cargo.toml
# hfs-core/Cargo.toml, hfs-schema/Cargo.toml, hfs/Cargo.toml → version = "0.1.0"

git add .
git commit -m "chore: release v0.1.0"
git tag -a v0.1.0 -m "v0.1.0 — MVP: ls, du, stat, schema Parquet, health"
git push origin main --tags

# Build binari statici per distribuzione
cargo build --release --target x86_64-unknown-linux-musl
cargo build --release --target aarch64-apple-darwin

# Upload su GitHub Releases (manuale o via gh CLI)
gh release create v0.1.0 \
  target/x86_64-unknown-linux-musl/release/hfs \
  target/aarch64-apple-darwin/release/hfs \
  --title "hfs v0.1.0" \
  --notes "MVP release: HDFS filesystem inspection without JVM"
```

---

## Comandi utili di sviluppo quotidiano

```bash
# Build veloce (dev mode, no ottimizzazioni)
cargo build -p hfs

# Test solo un crate
cargo test -p hfs-core
cargo test -p hfs-schema

# Run con log di debug
RUST_LOG=debug cargo run --bin hfs -- ls /

# Controlla dipendenze aggiornabili
cargo outdated

# Audit sicurezza
cargo audit

# Dimensione binario release
cargo build --release && ls -lh target/release/hfs

# Cross-compile per Linux su macOS
cross build --release --target x86_64-unknown-linux-musl
# (richiede `cargo install cross` e Docker)
```

---

## Checklist prima di ogni PR upstream

- [ ] I commit sono atomici e hanno messaggi descrittivi (no "fix stuff", no "WIP")
- [ ] Nessun riferimento a "hfs", "Davide", o strumenti AI nel codice/commit/PR
- [ ] Test aggiunti per la nuova funzionalità/fix
- [ ] `cargo test` passa sul fork
- [ ] `cargo clippy -- -D warnings` senza errori
- [ ] Il codice segue lo stile del repo originale (usa `cargo fmt`)
- [ ] Ho letto il `CONTRIBUTING.md` del repo upstream (se esiste)
- [ ] Ho verificato che non ci siano issue aperti simili già in corso
- [ ] Sono pronto a rispondere a domande tecniche sulla PR in inglese
