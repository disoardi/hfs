# Installazione

## Installazione rapida

```bash
curl -fsSL https://raw.githubusercontent.com/disoardi/hfs/main/install.sh | sh
```

Lo script rileva automaticamente il sistema operativo e l'architettura,
scarica il binario precompilato dalla pagina Release e lo installa in
`/usr/local/bin/hfs` (o `~/.local/bin/hfs` se non hai i permessi di root).

!!! tip "TuxBox"
    Se usi [TuxBox](https://github.com/disoardi/tuxbox) puoi installare hfs
    direttamente dal registry pubblico:
    ```bash
    tbox install hfs
    tbox run hfs -- ls /
    ```

---

## Download binario precompilato

I binari sono disponibili nella [pagina Release](https://github.com/disoardi/hfs/releases) del repository.

=== "Linux x86_64 (statico, musl)"

    Binario completamente statico, funziona su qualsiasi distribuzione Linux
    senza dipendenze di sistema.

    ```bash
    curl -L https://github.com/disoardi/hfs/releases/latest/download/hfs-linux-x86_64 \
      -o /usr/local/bin/hfs
    chmod +x /usr/local/bin/hfs
    hfs --version
    ```

=== "macOS ARM64"

    ```bash
    curl -L https://github.com/disoardi/hfs/releases/latest/download/hfs-macos-arm64 \
      -o /usr/local/bin/hfs
    chmod +x /usr/local/bin/hfs
    hfs --version
    ```

## Build da sorgente

Richiede [Rust](https://rustup.rs/) stable.

```bash
git clone https://github.com/disoardi/hfs.git
cd hfs
cargo build --release
# Il binario è in: target/release/hfs
```

### Build statico (Linux musl)

```bash
rustup target add x86_64-unknown-linux-musl
# Su Ubuntu/Debian: sudo apt install musl-tools
cargo build --release --target x86_64-unknown-linux-musl
# Il binario è in: target/x86_64-unknown-linux-musl/release/hfs
```

### Build con supporto Kerberos

```bash
# Richiede libgssapi-dev sul sistema
# Ubuntu: sudo apt install libgssapi-krb5-2 libkrb5-dev
cargo build --release --features kerberos
```

!!! note "Kerberos e build musl"
    Il build con `--features kerberos` non è compatibile con il target musl
    perché `libgssapi-sys` richiede la libreria di sistema. Usa il target
    `x86_64-unknown-linux-gnu` con Kerberos.

## Verifica installazione

```bash
hfs --version
# hfs 0.1.0

# Test rapido con WebHDFS (se disponibile)
hfs --namenode http://your-namenode:9870 --backend webhdfs ls /
```
