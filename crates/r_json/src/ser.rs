//! `ToJson` trait and primitive implementations.

pub trait ToJson {
    fn json_write(&self, w: &mut Vec<u8>);

    /// Hint for the approximate number of bytes this value will serialize to.
    ///
    /// This is used by `to_json_bytes` to pre-allocate the output buffer,
    /// avoiding reallocations for the common case.  Implementations should
    /// return a value that is *at least* as large as the serialized form in
    /// the common case — over-estimating is fine, under-estimating causes a
    /// single reallocation.  The default (64) is conservative.
    #[inline]
    fn json_size_hint(&self) -> usize { 64 }

    fn to_json_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.json_size_hint());
        self.json_write(&mut buf);
        buf
    }

    fn to_json_string(&self) -> String {
        String::from_utf8(self.to_json_bytes())
            .expect("ToJson implementations always emit valid UTF-8")
    }
}

// ── SWAR string escaping helpers ─────────────────────────────────────────────
//
// Process 8 bytes at a time to detect characters that need JSON escaping.
// Three categories need escaping:
//   a) '"'  (0x22)
//   b) '\\' (0x5C)
//   c) control chars: any byte < 0x20

#[inline(always)]
const fn swar_has_byte_eq(x: u64, target: u8) -> u64 {
    let rep = 0x0101_0101_0101_0101_u64.wrapping_mul(target as u64);
    let v = x ^ rep;
    v.wrapping_sub(0x0101_0101_0101_0101_u64) & !v & 0x8080_8080_8080_8080_u64
}

/// Returns a mask with 0x80 set at each byte position where byte < 0x20
/// (i.e., control characters that need \uXXXX or shorthand escaping).
#[inline(always)]
const fn swar_has_ctrl(x: u64) -> u64 {
    // Mask to top 3 bits of each byte; ctrl chars have top 3 bits = 0b000
    let masked = x & 0xE0E0_E0E0_E0E0_E0E0_u64;
    // has-zero-byte on the masked value detects ctrl chars
    masked.wrapping_sub(0x0101_0101_0101_0101_u64) & !masked & 0x8080_8080_8080_8080_u64
}

/// Combined escape-needed mask for an 8-byte chunk.
/// Returns 0 if all 8 bytes can be copied verbatim.
#[inline(always)]
fn swar_needs_escape(x: u64) -> u64 {
    swar_has_byte_eq(x, b'"') | swar_has_byte_eq(x, b'\\') | swar_has_ctrl(x)
}

// ── 16-byte u128 SWAR helpers (active when `simd` feature is enabled) ────────

#[cfg(feature = "simd")]
#[inline(always)]
const fn swar128_has_byte_eq(x: u128, target: u8) -> u128 {
    let rep = 0x0101_0101_0101_0101_0101_0101_0101_0101_u128.wrapping_mul(target as u128);
    let v = x ^ rep;
    v.wrapping_sub(0x0101_0101_0101_0101_0101_0101_0101_0101_u128)
        & !v
        & 0x8080_8080_8080_8080_8080_8080_8080_8080_u128
}

/// Returns a mask with 0x80 set at each byte position where byte < 0x20
/// (control characters) — 16-byte variant.
#[cfg(feature = "simd")]
#[inline(always)]
const fn swar128_has_ctrl(x: u128) -> u128 {
    let masked = x & 0xE0E0_E0E0_E0E0_E0E0_E0E0_E0E0_E0E0_E0E0_u128;
    masked.wrapping_sub(0x0101_0101_0101_0101_0101_0101_0101_0101_u128)
        & !masked
        & 0x8080_8080_8080_8080_8080_8080_8080_8080_u128
}

/// Combined escape-needed mask for a 16-byte chunk.
/// Returns 0 if all 16 bytes can be copied verbatim.
#[cfg(feature = "simd")]
#[inline(always)]
fn swar128_needs_escape(x: u128) -> u128 {
    swar128_has_byte_eq(x, b'"') | swar128_has_byte_eq(x, b'\\') | swar128_has_ctrl(x)
}

// ── string escaping ───────────────────────────────────────────────────────────

#[inline]
pub fn write_escaped_str(s: &str, w: &mut Vec<u8>) {
    w.push(b'"');
    // Pre-reserve: common case is no escaping, so reserve s.len() + 1 (closing quote).
    // Avoids all reallocations in the fast (no-escape) path.
    w.reserve(s.len() + 1);
    let bytes = s.as_bytes();
    let mut start = 0usize;  // start of current safe run
    let mut i = 0usize;

    // ── 16-byte u128 SWAR loop (active when `simd` feature is enabled) ────────
    // The compiler will typically auto-vectorize this to NEON/SSE2, halving
    // iteration count vs. the 8-byte loop (3 iters vs. 6 for a 50-char string).
    #[cfg(feature = "simd")]
    {
        while i + 16 <= bytes.len() {
            let chunk = {
                let mut b = [0u8; 16];
                b.copy_from_slice(&bytes[i..i + 16]);
                u128::from_le_bytes(b)
            };
            let mask = swar128_needs_escape(chunk);
            if mask == 0 {
                i += 16;
                continue;
            }
            let escape_pos = i + (mask.trailing_zeros() / 8) as usize;
            w.extend_from_slice(&bytes[start..escape_pos]);
            escape_one(bytes[escape_pos], w);
            i = escape_pos + 1;
            start = i;
        }
        // Fall through to 8-byte loop for the remaining < 16 bytes.
    }

    // ── 8-byte u64 SWAR loop (always available) ────────────────────────────
    while i + 8 <= bytes.len() {
        let chunk = u64::from_le_bytes(
            bytes[i..i + 8].try_into().expect("8 bytes"),
        );
        let mask = swar_needs_escape(chunk);
        if mask == 0 {
            // All 8 bytes are safe — advance without touching `w`.
            i += 8;
            continue;
        }
        // First byte needing escape is at this position within the chunk.
        let escape_pos = i + (mask.trailing_zeros() / 8) as usize;
        // Flush everything before the escape character.
        w.extend_from_slice(&bytes[start..escape_pos]);
        // Emit the escape sequence for bytes[escape_pos].
        escape_one(bytes[escape_pos], w);
        i = escape_pos + 1;
        start = i;
    }

    // ── Scalar tail: handle remaining < 8 bytes ────────────────────────────
    while i < bytes.len() {
        let b = bytes[i];
        if needs_escape(b) {
            w.extend_from_slice(&bytes[start..i]);
            escape_one(b, w);
            start = i + 1;
        }
        i += 1;
    }

    // Flush the final safe run.
    w.extend_from_slice(&bytes[start..]);
    w.push(b'"');
}

#[inline(always)]
fn needs_escape(b: u8) -> bool {
    b == b'"' || b == b'\\' || b < 0x20
}

#[inline(always)]
fn escape_one(b: u8, w: &mut Vec<u8>) {
    match b {
        b'"'  => w.extend_from_slice(b"\\\""),
        b'\\' => w.extend_from_slice(b"\\\\"),
        b'\n' => w.extend_from_slice(b"\\n"),
        b'\r' => w.extend_from_slice(b"\\r"),
        b'\t' => w.extend_from_slice(b"\\t"),
        0x08  => w.extend_from_slice(b"\\b"),
        0x0C  => w.extend_from_slice(b"\\f"),
        b     => {
            // Other control characters as \u00XX
            let hi = b >> 4;
            let lo = b & 0xF;
            w.extend_from_slice(&[
                b'\\', b'u', b'0', b'0',
                if hi < 10 { b'0' + hi } else { b'a' + hi - 10 },
                if lo < 10 { b'0' + lo } else { b'a' + lo - 10 },
            ]);
        }
    }
}

// ── primitive impls ───────────────────────────────────────────────────────────

impl ToJson for bool {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) {
        w.extend_from_slice(if *self { b"true" } else { b"false" });
    }
    #[inline] fn json_size_hint(&self) -> usize { 5 } // "false"
}

impl ToJson for str {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) { write_escaped_str(self, w); }
    #[inline] fn json_size_hint(&self) -> usize { self.len() + 2 }
}

impl ToJson for String {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) { write_escaped_str(self, w); }
    #[inline] fn json_size_hint(&self) -> usize { self.len() + 2 }
}

impl<T: ToJson + ?Sized> ToJson for &T {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) { (**self).json_write(w); }
    #[inline] fn json_size_hint(&self) -> usize { (**self).json_size_hint() }
}

impl<T: ToJson> ToJson for Box<T> {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) { (**self).json_write(w); }
    #[inline] fn json_size_hint(&self) -> usize { (**self).json_size_hint() }
}

impl<T: ToJson> ToJson for Option<T> {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) {
        match self {
            Some(v) => v.json_write(w),
            None    => w.extend_from_slice(b"null"),
        }
    }
    #[inline]
    fn json_size_hint(&self) -> usize {
        match self {
            Some(v) => v.json_size_hint(),
            None    => 4, // "null"
        }
    }
}

impl<T: ToJson> ToJson for Vec<T> {
    fn json_write(&self, w: &mut Vec<u8>) {
        w.push(b'[');
        let mut first = true;
        for item in self {
            if !first { w.push(b','); }
            item.json_write(w);
            first = false;
        }
        w.push(b']');
    }
    #[inline]
    fn json_size_hint(&self) -> usize {
        if self.is_empty() { return 2; }
        // Use the first element's hint as a sample; add separating commas.
        2 + self.len() * (self[0].json_size_hint() + 1)
    }
}

impl<T: ToJson, const N: usize> ToJson for [T; N] {
    fn json_write(&self, w: &mut Vec<u8>) {
        w.push(b'[');
        let mut first = true;
        for item in self {
            if !first { w.push(b','); }
            item.json_write(w);
            first = false;
        }
        w.push(b']');
    }
    #[inline]
    fn json_size_hint(&self) -> usize {
        if N == 0 { return 2; }
        2 + N * (self[0].json_size_hint() + 1)
    }
}

impl<T: ToJson> ToJson for [T] {
    fn json_write(&self, w: &mut Vec<u8>) {
        w.push(b'[');
        let mut first = true;
        for item in self {
            if !first { w.push(b','); }
            item.json_write(w);
            first = false;
        }
        w.push(b']');
    }
    #[inline]
    fn json_size_hint(&self) -> usize {
        if self.is_empty() { return 2; }
        2 + self.len() * (self[0].json_size_hint() + 1)
    }
}

// Note: A specialized impl for Vec<f64> is not possible on stable Rust due to
// the coherence rules (conflicts with impl<T: ToJson> ToJson for Vec<T>).
// The generic Vec<T> impl with json_size_hint delegating to f64::json_size_hint (10)
// covers the f64 case correctly through monomorphization.

// ── integer writers (no format! overhead) ─────────────────────────────────────

#[inline(always)]
pub fn write_u64(mut n: u64, w: &mut Vec<u8>) {
    if n == 0 { w.push(b'0'); return; }
    let mut tmp = [0u8; 20];
    let mut len = 0usize;
    while n > 0 { tmp[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; }
    tmp[..len].reverse();
    w.extend_from_slice(&tmp[..len]);
}

#[inline(always)]
pub fn write_i64(n: i64, w: &mut Vec<u8>) {
    if n < 0 { w.push(b'-'); write_u64(n.unsigned_abs(), w); } else { write_u64(n as u64, w); }
}

macro_rules! impl_uint {
    ($($t:ty, $hint:expr),*) => {$(
        impl ToJson for $t {
            #[inline] fn json_write(&self, w: &mut Vec<u8>) { write_u64(*self as u64, w); }
            #[inline] fn json_size_hint(&self) -> usize { $hint }
        }
    )*};
}
macro_rules! impl_sint {
    ($($t:ty, $hint:expr),*) => {$(
        impl ToJson for $t {
            #[inline] fn json_write(&self, w: &mut Vec<u8>) { write_i64(*self as i64, w); }
            #[inline] fn json_size_hint(&self) -> usize { $hint }
        }
    )*};
}
// Tight upper bounds (max digit count including sign for signed types):
//   u8:3, u16:5, u32:10, u64:20, u128:39, usize:20
//   i8:4, i16:6, i32:11, i64:20, i128:40, isize:20
impl_uint!(u8, 3, u16, 5, u32, 10, u64, 20, u128, 39, usize, 20);
impl_sint!(i8, 4, i16, 6, i32, 11, i64, 20, i128, 40, isize, 20);

impl ToJson for f64 {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) {
        if !self.is_finite() { w.extend_from_slice(b"null"); return; }
        #[cfg(feature = "fast-float")]
        {
            let mut buf = ryu::Buffer::new();
            w.extend_from_slice(buf.format_finite(*self).as_bytes());
            return;
        }
        #[cfg(not(feature = "fast-float"))]
        w.extend_from_slice(format!("{}", self).as_bytes());
    }
    /// ryu's worst-case output for f64 is 24 characters, but the practical output for
    /// typical floats (integers, short decimals, small exponents) is 2–6 characters.
    /// Using 10 as the hint covers the vast majority of real-world floats without the
    /// 24-byte worst-case causing 96-byte allocations for small structs.  Under-estimation
    /// only causes a single reallocation, whereas over-estimation wastes allocator headroom
    /// and pushes small structs into larger (slower) allocator size classes.
    #[inline] fn json_size_hint(&self) -> usize { 10 }
}

impl ToJson for f32 {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) {
        if !self.is_finite() { w.extend_from_slice(b"null"); return; }
        #[cfg(feature = "fast-float")]
        {
            let mut buf = ryu::Buffer::new();
            w.extend_from_slice(buf.format_finite(*self).as_bytes());
            return;
        }
        #[cfg(not(feature = "fast-float"))]
        w.extend_from_slice(format!("{}", self).as_bytes());
    }
    /// ryu's output for f32 is at most 14 characters.
    #[inline] fn json_size_hint(&self) -> usize { 14 }
}
