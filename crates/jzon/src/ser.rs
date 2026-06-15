//! `ToJson` trait and primitive implementations.

use std::collections::{BTreeMap, HashMap};

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

    #[must_use]
    fn to_json_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.json_size_hint());
        self.json_write(&mut buf);
        buf
    }

    #[must_use]
    fn to_json_string(&self) -> String {
        // SAFETY: ToJson implementations only write valid UTF-8 bytes.
        // The expect path is unreachable for any correct impl; using expect (not
        // unwrap_unchecked) to preserve a clear panic message for buggy custom impls.
        String::from_utf8(self.to_json_bytes())
            .expect("ToJson implementations always emit valid UTF-8")
    }
}

// ── string escaping ───────────────────────────────────────────────────────────
//
// `write_escaped_str` delegates byte scanning to `crate::simd::find_escape`,
// which dispatches to the widest available implementation:
//   - nightly + simd feature  → 32-byte std::simd lanes  (find_escape_simd32)
//   - stable + simd feature   → 16-byte u128 SWAR        (find_escape_simd16)
//   - no simd feature         → scalar byte-by-byte      (find_escape_scalar)
//
// This keeps ser.rs free of SWAR arithmetic — all the bit tricks live in simd.rs.

#[inline]
pub fn write_escaped_str(s: &str, w: &mut Vec<u8>) {
    w.push(b'"');
    // Pre-reserve: common case is no escaping, so reserve s.len() + 1 (closing quote).
    // Avoids all reallocations in the fast (no-escape) path.
    w.reserve(s.len() + 1);
    let bytes = s.as_bytes();
    let mut start = 0usize; // start of current unescaped run

    let mut i = start;
    while i < bytes.len() {
        // Find the next byte that needs escaping using the widest available path.
        let stop = crate::simd::find_escape(bytes, i);
        if stop >= bytes.len() {
            // No more bytes need escaping; flush the rest in one go.
            break;
        }
        // Flush safe bytes [start..stop], then emit the escape sequence.
        w.extend_from_slice(&bytes[start..stop]);
        escape_one(bytes[stop], w);
        i = stop + 1;
        start = i;
    }

    // Flush the final safe run.
    w.extend_from_slice(&bytes[start..]);
    w.push(b'"');
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
impl_uint!(u8, 3, u16, 5, u32, 10, u64, 20, usize, 20);
impl_sint!(i8, 4, i16, 6, i32, 11, i64, 20, isize, 20);

// u128 / i128: cannot pass through u64/i64, need dedicated digit writers.
#[inline]
fn write_u128(mut n: u128, w: &mut Vec<u8>) {
    if n == 0 { w.push(b'0'); return; }
    let mut tmp = [0u8; 39];
    let mut len = 0usize;
    while n > 0 { tmp[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; }
    tmp[..len].reverse();
    w.extend_from_slice(&tmp[..len]);
}
impl ToJson for u128 {
    #[inline] fn json_write(&self, w: &mut Vec<u8>) { write_u128(*self, w); }
    #[inline] fn json_size_hint(&self) -> usize { 39 }
}
impl ToJson for i128 {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) {
        if *self < 0 { w.push(b'-'); write_u128(self.unsigned_abs(), w); } else { write_u128(*self as u128, w); }
    }
    #[inline] fn json_size_hint(&self) -> usize { 40 }
}

impl ToJson for f64 {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) {
        if !self.is_finite() { w.extend_from_slice(b"null"); return; }
        #[cfg(feature = "zmij-float-ser")]
        {
            let mut buf = zmij::Buffer::new();
            w.extend_from_slice(buf.format_finite(*self).as_bytes());
            return;
        }
        #[cfg(all(feature = "fast-float", not(feature = "zmij-float-ser")))]
        {
            let mut buf = ryu::Buffer::new();
            w.extend_from_slice(buf.format_finite(*self).as_bytes());
            return;
        }
        #[cfg(not(any(feature = "fast-float", feature = "zmij-float-ser")))]
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
        #[cfg(feature = "zmij-float-ser")]
        {
            let mut buf = zmij::Buffer::new();
            w.extend_from_slice(buf.format_finite(*self).as_bytes());
            return;
        }
        #[cfg(all(feature = "fast-float", not(feature = "zmij-float-ser")))]
        {
            let mut buf = ryu::Buffer::new();
            w.extend_from_slice(buf.format_finite(*self).as_bytes());
            return;
        }
        #[cfg(not(any(feature = "fast-float", feature = "zmij-float-ser")))]
        w.extend_from_slice(format!("{}", self).as_bytes());
    }
    /// ryu's output for f32 is at most 14 characters.
    #[inline] fn json_size_hint(&self) -> usize { 14 }
}

// ── char ──────────────────────────────────────────────────────────────────────

impl ToJson for char {
    #[inline]
    fn json_write(&self, w: &mut Vec<u8>) {
        let mut buf = [0u8; 4];
        write_escaped_str(self.encode_utf8(&mut buf), w);
    }
    /// At most 4 UTF-8 bytes + 2 surrounding quotes.
    #[inline] fn json_size_hint(&self) -> usize { 6 }
}

// ── unit → null ───────────────────────────────────────────────────────────────

impl ToJson for () {
    #[inline] fn json_write(&self, w: &mut Vec<u8>) { w.extend_from_slice(b"null"); }
    #[inline] fn json_size_hint(&self) -> usize { 4 }
}

// ── HashMap / BTreeMap → JSON objects ────────────────────────────────────────

impl<K: ToJson, V: ToJson> ToJson for HashMap<K, V> {
    fn json_write(&self, w: &mut Vec<u8>) {
        w.push(b'{');
        let mut first = true;
        for (k, v) in self {
            if !first { w.push(b','); }
            first = false;
            k.json_write(w);
            w.push(b':');
            v.json_write(w);
        }
        w.push(b'}');
    }
    #[inline]
    fn json_size_hint(&self) -> usize {
        if self.is_empty() { return 2; }
        let (k, v) = self.iter().next().unwrap();
        2 + self.len() * (k.json_size_hint() + 1 + v.json_size_hint() + 1)
    }
}

impl<K: ToJson, V: ToJson> ToJson for BTreeMap<K, V> {
    fn json_write(&self, w: &mut Vec<u8>) {
        w.push(b'{');
        let mut first = true;
        for (k, v) in self {
            if !first { w.push(b','); }
            first = false;
            k.json_write(w);
            w.push(b':');
            v.json_write(w);
        }
        w.push(b'}');
    }
    #[inline]
    fn json_size_hint(&self) -> usize {
        if self.is_empty() { return 2; }
        let (k, v) = self.iter().next().unwrap();
        2 + self.len() * (k.json_size_hint() + 1 + v.json_size_hint() + 1)
    }
}

// ── tuples → JSON arrays (1- to 12-element) ───────────────────────────────────

macro_rules! impl_tuple_to_json {
    ($($T:ident . $idx:tt),+) => {
        impl<$($T: ToJson),+> ToJson for ($($T,)+) {
            fn json_write(&self, w: &mut Vec<u8>) {
                w.push(b'[');
                let mut first = true;
                $( if !first { w.push(b','); } first = false; self.$idx.json_write(w); )+
                let _ = first;
                w.push(b']');
            }
            #[inline]
            fn json_size_hint(&self) -> usize {
                2 + $( self.$idx.json_size_hint() + 1 + )+ 0
                  - 1 // subtract trailing extra comma count
            }
        }
    };
}

impl_tuple_to_json!(A.0);
impl_tuple_to_json!(A.0, B.1);
impl_tuple_to_json!(A.0, B.1, C.2);
impl_tuple_to_json!(A.0, B.1, C.2, D.3);
impl_tuple_to_json!(A.0, B.1, C.2, D.3, E.4);
impl_tuple_to_json!(A.0, B.1, C.2, D.3, E.4, F.5);
impl_tuple_to_json!(A.0, B.1, C.2, D.3, E.4, F.5, G.6);
impl_tuple_to_json!(A.0, B.1, C.2, D.3, E.4, F.5, G.6, H.7);
impl_tuple_to_json!(A.0, B.1, C.2, D.3, E.4, F.5, G.6, H.7, I.8);
impl_tuple_to_json!(A.0, B.1, C.2, D.3, E.4, F.5, G.6, H.7, I.8, J.9);
impl_tuple_to_json!(A.0, B.1, C.2, D.3, E.4, F.5, G.6, H.7, I.8, J.9, K.10);
impl_tuple_to_json!(A.0, B.1, C.2, D.3, E.4, F.5, G.6, H.7, I.8, J.9, K.10, L.11);
