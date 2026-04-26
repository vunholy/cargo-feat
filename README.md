# cargo-feat

[![crates.io](https://img.shields.io/crates/v/cargo-feat.svg)](https://crates.io/crates/cargo-feat)

> **Disclaimer:** I am not a Rust professional and I am not claiming to be one - I built this simply because it was useful to me. The core logic was written by me; Some of the performance work and optimizations were done in collaboration with Claude.

A fast command-line tool for Rust developers to instantly look up the available features of any crate on [crates.io](https://crates.io) - directly in your terminal, with no browser required.

---

## Why

When adding a dependency, finding out what features it exposes usually means opening a browser, searching crates.io or docs.rs, and scrolling through documentation. `cargo-feat` removes that friction: one command gives you a color-coded list of every feature the crate exposes, which ones are enabled by default, and what each feature pulls in.

---

## Demo

```
$ feat reqwest

— reqwest's features are in the following list —
    ★ default
         default-tls
         charset
         http2
         system-proxy

    — blocking
    — brotli
    — charset          (default)
    — cookies
    — default-tls      (default)
    — deflate
    — gzip
    — http2            (default)
    — ...
```

---

## Installation

### From crates.io

```sh
cargo install cargo-feat
```

### From source

Requires a recent stable [Rust toolchain](https://rustup.rs/).

```sh
git clone https://github.com/vunholy/cargo-feat.git
cd cargo-feat
cargo build --release
```

The compiled binary will be at `target/release/feat` (or `feat.exe` on Windows).

To make it available globally, copy it to a directory in your `PATH`:

```sh
# Linux / macOS
cp target/release/feat ~/.local/bin/feat

# Windows (PowerShell - adjust path as needed)
Copy-Item target\release\feat.exe "$env:USERPROFILE\.cargo\bin\feat.exe"
```

---

## Usage

```
feat <crate-name> [version] [all|nd]
```

| Argument | Required | Description |
|---|---|---|
| `<crate-name>` | Yes | Name of the crate to look up. Underscores are automatically normalized to hyphens. |
| `[all\|nd]` | No | Feature filter. `all` (default) shows every feature. `nd` hides the default feature block but still lists all features and marks the ones that are part of the default set. |
| `[version]` | No | Specific crate version to query. Defaults to the latest stable release. |

The `version` and `all|nd` arguments are order-independent — `feat tokio 1.35.0 nd` and `feat tokio nd 1.35.0` both work.

### Examples

```sh
# List all features for the latest version of reqwest
feat reqwest

# List features without the default block
feat reqwest nd

# List features for a specific version
feat tokio 1.35.0

# Combine a version with a filter
feat tokio 1.35.0 nd

# Crate names with underscores work fine
feat proc_macro2
```

---

## Output Format

```
— crate's features are in the following list —
    ★ default          <- the default feature set (lists which features it enables)
         feature-a
         feature-b

    — feature-a  (default)   <- enabled by default
    — feature-b  (default)
    — feature-c              <- opt-in feature
    — feature-d
```

- The `★ default` block lists which features are enabled when you add the crate without specifying features.
- Features marked `(default)` are part of the default feature set.
- Using `nd` hides the `★ default` block but keeps all features in the list, still marking the ones that belong to the default set.
- Internal features (names starting with `__`) are always hidden.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `103` | Crate not found or bad response |
| `104` | The specified version was not found for the crate |
| `105` | No stable version found for the crate |

---

## Performance

`cargo-feat` is built with performance as a first-class concern. Benchmarked against `cargo info`:

```
hyperfine --warmup 3 --runs 20 \
  -n 'cargo info (warm/offline)' 'cargo info reqwest --offline' \
  -n 'cargo info (network)'      'cargo info reqwest' \
  -n 'cargo feat'                'cargo feat reqwest'

  cargo feat                51.1 ms ±  1.8 ms
  cargo info (warm/offline) 81.9 ms ±  1.7 ms   1.6x slower
  cargo info (network)     360.5 ms ± 10.4 ms   7.1x slower
```

**How it achieves this:**

- **Sparse registry index** — reads directly from `index.crates.io` (Cloudflare CDN), the same source Cargo uses, instead of the heavier crates.io REST API.
- **Local cache first** — on every lookup, `cargo-feat` checks your local Cargo registry cache (`~/.cargo/registry/index/`) before touching the network. If you've ever built a project that depends on the crate, the result is instant. After any network fetch, the result is written back to the cache, so the next run is always fast.
- **[MiMalloc](https://github.com/microsoft/mimalloc)** — Microsoft's high-performance memory allocator replaces the system allocator globally.
- **[simd-json](https://github.com/simd-lite/simd-json)** — SIMD-accelerated JSON parsing (AVX2/FMA when available).
- **[hashbrown](https://github.com/rust-lang/hashbrown)** + **[ahash](https://github.com/tkaitchuck/ahash)** — faster `HashMap` and `HashSet` than the standard library.
- **Aggressive release profile** — fat LTO, single codegen unit, `opt-level = 3`, stripped symbols.
- **[ureq](https://github.com/algesten/ureq)** with rustls (pure-Rust TLS) and gzip decompression.

---

## Dependencies

| Crate | Purpose |
|---|---|
| `ureq` | Lightweight HTTP client with rustls TLS and gzip support |
| `simd-json` | SIMD-accelerated JSON deserialization |
| `serde` | Derive macros for JSON deserialization |
| `mimalloc` | Global high-performance memory allocator |
| `hashbrown` | Fast `HashMap` / `HashSet` backed by Swiss tables |
| `ahash` | Non-cryptographic, high-speed hash function |
| `colorize` | ANSI escape code helpers for colored terminal output |

---

## Build Configuration

The `.cargo/config.toml` enables additional compiler flags for the Windows MSVC target:

- `target-cpu=native` — compile for the host CPU to unlock all supported ISA extensions.
- `target-feature=+avx2,+fma` — explicitly enable AVX2 and FMA for simd-json.
- `rust-lld.exe` — LLVM linker for faster link times.
- `/OPT:REF`, `/OPT:ICF` — dead-code elimination and identical-COMDAT-folding at link time.

If you are building for a different target or a CPU without AVX2, remove or adjust those flags in `.cargo/config.toml`.

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE-APACHE).
