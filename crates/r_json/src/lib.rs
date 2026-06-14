//! **r_json** — purpose-built, zero-copy JSON serialization for specific structs.
//!
//! # Design
//!
//! Use `#[derive(ToJson, FromJson)]` to generate a **monomorphised** JSON
//! (de)serializer for each of your types at compile time.  No generic visitor
//! indirection, no intermediate `Value` allocation, no format-string overhead.
//!
//! ## Cargo features
//!
//! | Feature | Effect |
//! |---------|--------|
//! | *(none)* | SWAR u64 scanning (8 B/iter, safe, zero deps) |
//! | `simd` | Adds u128 SWAR (16 B/iter) on all platforms |
//! | `simd + unstable` | Uses `std::simd` portable SIMD (32–64 B/iter) on nightly Rust |
//! | `stats` | Attaches a `ScannerStats` to every `Scanner` to track allocations and cache hits |
//!
//! ## Zero-copy deserialization
//!
//! Fields typed `&'de str` borrow **directly** from the input — no `String` is
//! allocated unless the JSON string contains escape sequences (in which case
//! `Error::EscapedString` is returned so the user can switch to `String`).
//!
//! ## Field-hint cache
//!
//! The generated `FromJson` impl maintains a one-word *field-hint* variable
//! that predicts which field key to expect next.  For JSON payloads whose
//! field order matches the struct definition — the common case — almost every
//! key dispatch is O(1) without hashing.
//!
//! ## Safe Rust only
//!
//! There are **no `unsafe` blocks** anywhere in this crate.  All SIMD scanning
//! is done through `std::simd` (nightly) or pure u64/u128 arithmetic (SWAR).
//!
//! # Quick start
//!
//! ```rust,ignore
//! use r_json::{ToJson, FromJson};
//!
//! #[derive(ToJson, FromJson, Debug, PartialEq)]
//! #[serde(rename_all = "camelCase")]
//! struct User<'a> {
//!     user_id:  u64,
//!     name:     &'a str,       // zero-copy borrow
//!     #[serde(skip_serializing_if = "Option::is_none")]
//!     email:    Option<String>,
//!     #[serde(default)]
//!     score:    f64,
//! }
//!
//! let input = r#"{"userId":1,"name":"alice","score":9.5}"#;
//! let user: User = User::from_json_str(input).unwrap();
//! let out = user.to_json_string();
//! ```

// Enable `std::simd` portable SIMD on nightly when `unstable` feature is set.
#![cfg_attr(all(feature = "simd", feature = "unstable"), feature(portable_simd))]

pub mod error;
pub mod scanner;
pub mod ser;
pub mod de;
pub mod simd;
#[cfg(feature = "stats")]
pub mod stats;

pub use error::Error;
pub use scanner::{JsonStr, Scanner};
pub use ser::ToJson;
pub use de::FromJson;

// Re-export the derive macros under the same names as the traits.
// In Rust, traits live in the type namespace and derive macros live in the
// macro namespace — they can share the same identifier without conflict.
pub use r_json_derive::{FromJson, ToJson};
