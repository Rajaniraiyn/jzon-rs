# jzon-rs-serde

[![crates.io](https://img.shields.io/crates/v/jzon-rs-serde.svg)](https://crates.io/crates/jzon-rs-serde)
[![docs.rs](https://docs.rs/jzon-rs-serde/badge.svg)](https://docs.rs/jzon-rs-serde)
[![MSRV](https://img.shields.io/badge/rustc-1.71%2B-blue.svg)](https://blog.rust-lang.org/2022/11/03/Rust-1.71.0.html)

SIMD-backed serde `Serializer`/`Deserializer` for any type deriving `serde::Serialize`/`serde::Deserialize`.

## Usage

```toml
[dependencies]
jzon-rs-serde = "0.2"
serde = { version = "1", features = ["derive"] }
```

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct User<'a> {
    id: u64,
    name: &'a str,   // zero-copy via visit_borrowed_str
}

fn main() {
    let src = r#"{"id":42,"name":"ada"}"#;
    let user: User = jzon_serde::from_str(src).unwrap();
    let out: String = jzon_serde::to_string(&user).unwrap();
    println!("{out}");
}
```

Zero-copy `&str` fields work transparently: the deserializer calls `visit_borrowed_str`, so the string data is borrowed directly from the input slice with no allocation.

## Feature Flags

Feature flags mirror those of [jzon-rs](https://crates.io/crates/jzon-rs).

| Flag | Default | Description |
|------|---------|-------------|
| `simd` | off | u128 SWAR (16 bytes/iter) scanning |
| `fast-float` | off | `ryu` float serialization, `fast_float2` parsing |
| `unstable` | off | `std::simd` portable SIMD 32–64 bytes/iter (nightly only) |
| `stats` | off | Allocation counters on the underlying Scanner |

## Part of the jzon family

| Crate | Purpose |
|-------|---------|
| [jzon-rs](https://crates.io/crates/jzon-rs) | Core zero-copy JSON with `#[derive(ToJson, FromJson)]` |
| [jzon-rs-compat](https://crates.io/crates/jzon-rs-compat) | Drop-in `serde_json` replacement |

## License

MIT
