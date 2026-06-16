// SWAR: has_byte(x, t) = has_zero(x ^ repeat(t)); has_zero(v) = (v - 0x0101…01) & !v & 0x8080…80
// Each result byte is 0x80 if the input byte equals target, zero otherwise.

#[inline(always)]
const fn swar_has_byte(x: u64, target: u8) -> u64 {
    let rep = 0x0101_0101_0101_0101_u64.wrapping_mul(target as u64);
    let v = x ^ rep;
    v.wrapping_sub(0x0101_0101_0101_0101_u64) & !v & 0x8080_8080_8080_8080_u64
}

/// Scan `input[start..]` for the first `"` or `\` byte.
/// Returns the index in `input`, or `input.len()` if none found.
pub fn find_quote_or_backslash(input: &[u8], start: usize) -> usize {
    let mut i = start;

    while i + 8 <= input.len() {
        let chunk = u64::from_le_bytes(
            input[i..i + 8].try_into().expect("slice is 8 bytes"),
        );
        let m = swar_has_byte(chunk, b'"') | swar_has_byte(chunk, b'\\');
        if m != 0 {
            return i + (m.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }

    while i < input.len() {
        if input[i] == b'"' || input[i] == b'\\' {
            return i;
        }
        i += 1;
    }
    i
}

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

    find_quote_or_backslash(input, i)
}

#[cfg(all(feature = "simd", feature = "unstable"))]
pub fn find_quote_or_backslash_portable32(input: &[u8], start: usize) -> usize {
    use std::simd::{cmp::SimdPartialEq, num::SimdUint, u8x32};

    let quote  = u8x32::splat(b'"');
    let slash  = u8x32::splat(b'\\');
    let mut i  = start;

    while i + 32 <= input.len() {
        let chunk = u8x32::from_slice(&input[i..i + 32]);
        let m = chunk.simd_eq(quote) | chunk.simd_eq(slash);
        let mask = m.to_bitmask();
        if mask != 0 {
            return i + mask.trailing_zeros() as usize;
        }
        i += 32;
    }

    find_quote_or_backslash_simd16(input, i)
}

/// 64-byte lanes — compiler emits AVX-512/SVE/etc. automatically; Rust code is fully safe.
#[cfg(all(feature = "simd", feature = "unstable"))]
pub fn find_quote_or_backslash_portable64(input: &[u8], start: usize) -> usize {
    use std::simd::{cmp::SimdPartialEq, num::SimdUint, u8x64};

    let quote  = u8x64::splat(b'"');
    let slash  = u8x64::splat(b'\\');
    let mut i  = start;

    while i + 64 <= input.len() {
        let chunk = u8x64::from_slice(&input[i..i + 64]);
        let m = chunk.simd_eq(quote) | chunk.simd_eq(slash);
        let mask = m.to_bitmask();
        if mask != 0 {
            return i + mask.trailing_zeros() as usize;
        }
        i += 64;
    }

    find_quote_or_backslash_portable32(input, i)
}

/// Dispatch to the widest safe implementation available for this build.
#[inline]
pub fn find(input: &[u8], start: usize) -> usize {
    #[cfg(all(feature = "simd-intrinsics", target_arch = "aarch64"))]
    return crate::simd_arch::neon::find_quote_or_backslash_64(input, start);

    #[cfg(all(feature = "simd-intrinsics", target_arch = "x86_64"))]
    return crate::simd_arch::x86::find_quote_or_backslash_32(input, start);

    #[cfg(all(
        feature = "simd",
        feature = "unstable",
        not(any(
            all(feature = "simd-intrinsics", target_arch = "aarch64"),
            all(feature = "simd-intrinsics", target_arch = "x86_64"),
        )),
    ))]
    return find_quote_or_backslash_portable64(input, start);

    #[cfg(all(
        feature = "simd",
        not(feature = "unstable"),
        not(any(
            all(feature = "simd-intrinsics", target_arch = "aarch64"),
            all(feature = "simd-intrinsics", target_arch = "x86_64"),
        )),
    ))]
    return find_quote_or_backslash_simd16(input, start);

    #[cfg(not(feature = "simd"))]
    find_quote_or_backslash(input, start)
}

#[cfg(feature = "simd")]
#[allow(dead_code)] // used on non-aarch64 / when simd-intrinsics is off
#[inline(always)]
const fn swar128_has_ctrl(x: u128) -> u128 {
    // Bytes < 0x20: top 3 bits all zero.
    let masked = x & 0xE0E0_E0E0_E0E0_E0E0_E0E0_E0E0_E0E0_E0E0_u128;
    masked.wrapping_sub(0x0101_0101_0101_0101_0101_0101_0101_0101_u128)
        & !masked
        & 0x8080_8080_8080_8080_8080_8080_8080_8080_u128
}

/// Scan `input[start..]` for the first byte needing JSON string escaping
/// (`"`, `\`, or any byte < 0x20) using 16-byte u128 SWAR.
#[cfg(feature = "simd")]
#[allow(dead_code)]
pub fn find_escape_simd16(input: &[u8], start: usize) -> usize {
    let mut i = start;

    while i + 16 <= input.len() {
        let chunk = {
            let mut b = [0u8; 16];
            b.copy_from_slice(&input[i..i + 16]);
            u128::from_le_bytes(b)
        };
        let m = swar128_has_byte(chunk, b'"')
            | swar128_has_byte(chunk, b'\\')
            | swar128_has_ctrl(chunk);
        if m != 0 {
            return i + (m.trailing_zeros() / 8) as usize;
        }
        i += 16;
    }

    find_escape_scalar(input, i)
}

#[allow(dead_code)]
#[inline]
pub fn find_escape_scalar(input: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < input.len() {
        let b = input[i];
        if b == b'"' || b == b'\\' || b < 0x20 {
            return i;
        }
        i += 1;
    }
    input.len()
}

/// Scan `input[start..]` for the first byte needing JSON string escaping using 32-byte portable SIMD.
#[cfg(all(feature = "simd", feature = "unstable"))]
fn find_escape_simd32(input: &[u8], start: usize) -> usize {
    use std::simd::{cmp::SimdPartialEq, cmp::SimdPartialOrd, u8x32};

    let quote     = u8x32::splat(b'"');
    let slash     = u8x32::splat(b'\\');
    let threshold = u8x32::splat(0x20u8);

    let mut i = start;
    while i + 32 <= input.len() {
        let chunk = u8x32::from_slice(&input[i..i + 32]);
        let needs_esc = chunk.simd_eq(quote)
            | chunk.simd_eq(slash)
            | chunk.simd_lt(threshold);
        let mask = needs_esc.to_bitmask();
        if mask != 0 {
            return i + mask.trailing_zeros() as usize;
        }
        i += 32;
    }
    find_escape_simd16(input, i)
}

/// Find the first byte needing JSON string escaping (`"`, `\`, or `< 0x20`).
#[inline]
pub fn find_escape(input: &[u8], start: usize) -> usize {
    #[cfg(all(feature = "simd-intrinsics", target_arch = "aarch64"))]
    return crate::simd_arch::neon::find_escape_64(input, start);

    #[cfg(all(feature = "simd-intrinsics", target_arch = "x86_64"))]
    return crate::simd_arch::x86::find_escape_32(input, start);

    #[cfg(all(
        feature = "simd",
        feature = "unstable",
        not(any(
            all(feature = "simd-intrinsics", target_arch = "aarch64"),
            all(feature = "simd-intrinsics", target_arch = "x86_64"),
        )),
    ))]
    return find_escape_simd32(input, start);

    #[cfg(all(
        feature = "simd",
        not(feature = "unstable"),
        not(any(
            all(feature = "simd-intrinsics", target_arch = "aarch64"),
            all(feature = "simd-intrinsics", target_arch = "x86_64"),
        )),
    ))]
    return find_escape_simd16(input, start);

    #[cfg(not(feature = "simd"))]
    find_escape_scalar(input, start)
}
