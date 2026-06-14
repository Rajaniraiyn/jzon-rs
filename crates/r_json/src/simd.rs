//! Fast byte scanning — entirely **safe Rust**, zero `unsafe` blocks.
//!
//! Three tiers, each a superset of the previous:
//!
//! | Tier | Required | Width | Technique |
//! |------|----------|-------|-----------|
//! | SWAR | always   | 8 B   | u64 arithmetic — no SIMD intrinsics at all |
//! | portable_simd (stable shim) | `simd` feature | 16 B | hand-rolled u128 SWAR |
//! | portable_simd (nightly)  | `simd + unstable` | 16–32–64 B | `std::simd` safe API |
//!
//! The scalar / SWAR path runs on every platform and requires no feature flags.
//! The nightly path uses `#![feature(portable_simd)]` declared in `lib.rs`.

// ── SWAR: 8 bytes at a time with u64 arithmetic ──────────────────────────────
//
// Classic "has zero byte" trick, generalised to an arbitrary target byte:
//   has_byte(x, t) = has_zero(x ^ repeat(t))
//   has_zero(v)    = (v - 0x0101…01) & !v & 0x8080…80
//
// Each byte position in the result is 0x80 if the corresponding byte of `x`
// equals `target`; zero otherwise.  We combine the masks for `"` and `\`
// with bitwise OR.

#[inline(always)]
const fn swar_has_byte(x: u64, target: u8) -> u64 {
    let rep = 0x0101_0101_0101_0101_u64.wrapping_mul(target as u64);
    let v = x ^ rep;
    v.wrapping_sub(0x0101_0101_0101_0101_u64) & !v & 0x8080_8080_8080_8080_u64
}

/// Scan `input[start..]` for the first `"` or `\` byte.
/// Returns the index in `input`, or `input.len()` if none found.
/// Uses u64 SWAR — 8 bytes per iteration, 100 % safe, runs everywhere.
pub fn find_quote_or_backslash(input: &[u8], start: usize) -> usize {
    let mut i = start;

    // 8-byte SWAR loop.
    while i + 8 <= input.len() {
        let chunk = u64::from_le_bytes(
            input[i..i + 8].try_into().expect("slice is 8 bytes"),
        );
        let m = swar_has_byte(chunk, b'"') | swar_has_byte(chunk, b'\\');
        if m != 0 {
            // Each hit contributes a set bit at position (byte_index * 8 + 7).
            // `trailing_zeros / 8` gives the first matching byte index.
            return i + (m.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }

    // Scalar tail for the remaining < 8 bytes.
    while i < input.len() {
        if input[i] == b'"' || input[i] == b'\\' {
            return i;
        }
        i += 1;
    }
    i
}

// ── Portable SIMD — 16 bytes using u128 SWAR (stable, no nightly needed) ─────
//
// Same technique as above but with u128, giving 16-byte chunks and zero
// additional dependencies.  Available whenever `feature = "simd"` is set.

#[cfg(feature = "simd")]
#[inline(always)]
const fn swar128_has_byte(x: u128, target: u8) -> u128 {
    let rep = 0x0101_0101_0101_0101_0101_0101_0101_0101_u128
        .wrapping_mul(target as u128);
    let v = x ^ rep;
    v.wrapping_sub(0x0101_0101_0101_0101_0101_0101_0101_0101_u128)
        & !v
        & 0x8080_8080_8080_8080_8080_8080_8080_8080_u128
}

#[cfg(feature = "simd")]
pub fn find_quote_or_backslash_simd16(input: &[u8], start: usize) -> usize {
    let mut i = start;

    // 16-byte u128 SWAR.
    // Use the `[0u8; 16]; copy_from_slice` pattern instead of try_into().unwrap()
    // — LLVM recognises this as an aligned 16-byte load on supported architectures.
    while i + 16 <= input.len() {
        let chunk = {
            let mut b = [0u8; 16];
            b.copy_from_slice(&input[i..i + 16]);
            u128::from_le_bytes(b)
        };
        let m = swar128_has_byte(chunk, b'"') | swar128_has_byte(chunk, b'\\');
        if m != 0 {
            return i + (m.trailing_zeros() / 8) as usize;
        }
        i += 16;
    }

    // SWAR 8-byte tail.
    find_quote_or_backslash(input, i)
}

// ── std::simd portable SIMD (nightly, 16 / 32 / 64-byte lanes) ───────────────
//
// `std::simd` is a safe, architecture-neutral SIMD API.  It is stabilised on
// nightly Rust and gated behind `#![feature(portable_simd)]` in `lib.rs`.
// All operations below are 100 % safe.  The compiler selects the best
// available instruction set (SSE2, AVX2, AVX-512, NEON, SVE, …) automatically.

#[cfg(all(feature = "simd", feature = "unstable"))]
pub fn find_quote_or_backslash_portable32(input: &[u8], start: usize) -> usize {
    use std::simd::{u8x32, SimdPartialEq, ToBitMask};

    let quote  = u8x32::splat(b'"');
    let slash  = u8x32::splat(b'\\');
    let mut i  = start;

    while i + 32 <= input.len() {
        let chunk = u8x32::from_slice(&input[i..i + 32]);
        let mask  = (chunk.simd_eq(quote) | chunk.simd_eq(slash)).to_bitmask();
        if mask != 0 {
            return i + mask.trailing_zeros() as usize;
        }
        i += 32;
    }

    // Delegate the tail to the 16-byte u128 SWAR path.
    find_quote_or_backslash_simd16(input, i)
}

/// 64-byte lanes — requires AVX-512 / SVE / etc. at the hardware level, but
/// the **Rust code** is fully safe; the compiler emits the right instructions.
#[cfg(all(feature = "simd", feature = "unstable"))]
pub fn find_quote_or_backslash_portable64(input: &[u8], start: usize) -> usize {
    use std::simd::{u8x64, SimdPartialEq, ToBitMask};

    let quote  = u8x64::splat(b'"');
    let slash  = u8x64::splat(b'\\');
    let mut i  = start;

    while i + 64 <= input.len() {
        let chunk = u8x64::from_slice(&input[i..i + 64]);
        let mask  = (chunk.simd_eq(quote) | chunk.simd_eq(slash)).to_bitmask();
        if mask != 0 {
            return i + mask.trailing_zeros() as usize;
        }
        i += 64;
    }

    find_quote_or_backslash_portable32(input, i)
}

// ── dispatch helper used by the scanner ──────────────────────────────────────

/// Choose the widest safe implementation available for this build.
#[inline]
pub fn find(input: &[u8], start: usize) -> usize {
    #[cfg(all(feature = "simd", feature = "unstable"))]
    return find_quote_or_backslash_portable64(input, start);

    #[cfg(all(feature = "simd", not(feature = "unstable")))]
    return find_quote_or_backslash_simd16(input, start);

    #[cfg(not(feature = "simd"))]
    find_quote_or_backslash(input, start)
}
