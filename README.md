# jzon

[![Crates.io](https://img.shields.io/crates/v/jzon.svg)](https://crates.io/crates/jzon)
[![Docs.rs](https://docs.rs/jzon/badge.svg)](https://docs.rs/jzon)
[![CI](https://github.com/rajaniraiyn/jzon/actions/workflows/ci.yml/badge.svg)](https://github.com/rajaniraiyn/jzon/actions)

Zero-copy JSON for Rust. A proc-macro generates a typed, monomorphised
parser and serializer per struct at compile time — no runtime dispatch,
no intermediate `Value`, no unnecessary allocations.

## Three modes

### A — custom derive (fastest)

```toml
[dependencies]
jzon = "0.1"
```

```rust
use jzon::{ToJson, FromJson};

#[derive(ToJson, FromJson)]
#[serde(rename_all = "camelCase")]
struct User<'a> {
    id:    u64,
    name:  &'a str,  // zero-copy: borrows directly from the input bytes
    score: f64,
}

let user = User::from_json_str(input)?;
let out  = user.to_json_string();
```

### B — any serde type

```toml
[dependencies]
jzon_serde = "0.1"
```

```rust
let user: User = jzon_serde::from_str(input)?;
let out = jzon_serde::to_string(&user)?;
```

### C — drop-in for serde_json

```toml
# Cargo.toml
[patch.crates-io]
serde_json = { path = "crates/jzon_compat" }
```

Every `serde_json::from_str` / `to_string` call — including inside
reqwest, axum, and other deps — routes through jzon automatically.

## Features

| Feature | What it does |
|---------|-------------|
| *(default)* | SWAR u64 string scanning (8 bytes/iter, no unsafe) |
| `simd` | u128 SWAR (16 bytes/iter) |
| `simd + unstable` | `std::simd` portable SIMD, 32–64 bytes/iter (nightly) |
| `fast-float` | ryu for serialization, fast_float2 for parsing |
| `stats` | per-parse allocation counters on Scanner |

## Benchmarks

macOS arm64, `--features simd,fast-float`

**Deserialization**

| | serde_json | sonic-rs | simd-json | jzon/A | jzon/B |
|-|-----------|---------|---------|-------|-------|
| twitter.json 617KB | 327µs | 345µs | 332µs | **316µs** ★ | 345µs |
| canada.json 2.2MB | 3.51ms | 3.03ms | 3.40ms | **2.43ms** ★ | — |
| citm_catalog 1.6MB | 945µs | 767µs | — | — | **545µs** ★ |
| micro Point 25B | 88ns | 75ns | 210ns | **41ns** ★ | 76ns |
| micro Record 52B | 79ns | 89ns | — | **74ns** ★ | 87ns |

**Serialization**

| | serde_json | sonic-rs | jzon/A |
|-|-----------|---------|-------|
| twitter.json 617KB | 28µs | 10.4µs | **10.2µs** ★ |
| micro Record | 61ns | 72ns | **50ns** ★ |

★ = fastest. jzon/A wins or ties on every benchmark except long-string
serialization where sonic-rs uses NEON SIMD at 16–32 bytes/iter.

## How it works

**Deserialization**: the derive macro generates a field-dispatch loop where
keys ≤ 8 bytes compare as a single `u64` (one instruction), and larger
structs use a compile-time minimal perfect hash for O(1) dispatch.
Float fields use `fast_float2::parse_partial` — one scan, not two.
`&'de str` fields borrow from the input with no allocation.

**Serialization**: field keys are `b"\"name\":"` byte literals (compile-time
constants). Integers use custom digit writers; floats use ryu.
String escaping uses SWAR u64/u128 arithmetic to bulk-copy safe byte runs.

**Serde layer**: `jzon_serde` implements `serde::Serializer/Deserializer`
backed by the same scanner. `visit_borrowed_str` propagates zero-copy
borrowing to any type deriving `serde::Deserialize`.

## Serde attributes supported

`rename`, `rename_all` (8 modes), `skip`, `skip_serializing`,
`skip_deserializing`, `skip_serializing_if`, `default`, `alias`,
`deny_unknown_fields`, `tag` (internally-tagged enums), `transparent`.

Types: all primitives, `String`, `&'de str`, `Option<T>`, `Vec<T>`,
`HashMap`, `BTreeMap`, `char`, `()`, tuples 1–12, `u128`/`i128`,
newtype structs, tuple structs, enum struct variants.

---

Made with ❤️ by [Rajaniraiyn](https://github.com/rajaniraiyn)
