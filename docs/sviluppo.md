# Sviluppo e contribuzione

## Setup ambiente di sviluppo

```bash
git clone https://github.com/disoardi/hfs.git
cd hfs

# Build
cargo build

# Test unit
cargo test --workspace

# Test integration (richiede Docker)
docker compose -f docker/docker-compose.test.yml up -d
HFS_NAMENODE=http://localhost:9870 cargo test --workspace -- --include-ignored
docker compose -f docker/docker-compose.test.yml down
```

## Documentazione locale

```bash
pip install mkdocs-material mkdocs-static-i18n
mkdocs serve
# Apri http://localhost:8000
```

## Struttura del progetto

Vedi `CLAUDE.md` nel repository per la documentazione completa dell'architettura.

## Come contribuire

1. Fai fork del repository
2. Crea un branch: `git checkout -b feat/mia-feature`
3. Sviluppa con test: `cargo test --workspace`
4. Verifica qualità: `cargo clippy -- -D warnings && cargo fmt --check`
5. Apri una Pull Request
