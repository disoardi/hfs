# Installation

## Quick Install

```bash
curl -fsSL https://raw.githubusercontent.com/disoardi/hfs/main/install.sh | sh
```

The script detects your OS and architecture automatically, downloads the prebuilt binary
from the GitHub Releases page, and installs it to `/usr/local/bin/hfs`
(or `~/.local/bin/hfs` if you don't have root access).

!!! tip "TuxBox"
    If you use [TuxBox](https://github.com/disoardi/tuxbox) you can install hfs
    directly from the public registry:
    ```bash
    tbox install hfs
    tbox run hfs -- ls /
    ```

---

## Prebuilt Binaries

Binaries are available on the [Releases page](https://github.com/disoardi/hfs/releases).

=== "Linux x86_64 (static, musl)"

    Fully static binary — works on any Linux distribution without system dependencies.

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

---

## Build from Source

Requires [Rust](https://rustup.rs/) stable.

```bash
git clone https://github.com/disoardi/hfs.git
cd hfs
cargo build --release
# Binary is at: target/release/hfs
```

### Static Build (Linux musl)

```bash
rustup target add x86_64-unknown-linux-musl
# Ubuntu/Debian: sudo apt install musl-tools
cargo build --release --target x86_64-unknown-linux-musl
# Binary is at: target/x86_64-unknown-linux-musl/release/hfs
```

### Build with Kerberos Support

```bash
# Requires libgssapi-dev on the system
# Ubuntu: sudo apt install libgssapi-krb5-2 libkrb5-dev
cargo build --release --features kerberos
```

!!! note "Kerberos and musl"
    Building with `--features kerberos` is not compatible with the musl target
    because `libgssapi-sys` requires system libraries. Use the
    `x86_64-unknown-linux-gnu` target with Kerberos.

---

## Verify Installation

```bash
hfs --version
# hfs 0.1.0

# Quick test with WebHDFS (if available)
hfs --namenode http://your-namenode:9870 --backend webhdfs ls /
```
