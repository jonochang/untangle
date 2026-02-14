# Installation

## From crates.io

```bash
cargo install untangle
```

## From Source

```bash
git clone https://github.com/user/untangle
cd untangle
cargo build --release
```

The binary will be at `target/release/untangle`.

## Using Nix

The repository includes a `flake.nix` with all build dependencies:

```bash
git clone https://github.com/user/untangle
cd untangle
nix develop
cargo build --release
```

The Nix dev shell provides the Rust toolchain (with clippy, rustfmt, rust-src), native build dependencies (pkg-config, cmake, openssl, libgit2), and dev tools (cargo-nextest, cargo-deny, mdbook).

## Pre-built Binaries

Pre-built binaries for Linux, macOS, and Windows are available on the [releases page](https://github.com/user/untangle/releases).

## Verifying the Installation

```bash
untangle --help
```

You should see the available subcommands: `analyze`, `diff`, `graph`, and `config`.
