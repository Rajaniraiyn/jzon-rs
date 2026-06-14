//! `FromJson<'de>` trait and primitive implementations.

use crate::{Error, Scanner};

// ── digit lookup table (branchless, no per-byte range-check) ─────────────────
//
// Maps any byte value → its decimal digit value (0-9 for b'0'..=b'9'; 0xFF
// for everything else).  We validate once at the end rather than branching on
// every byte, keeping the hot loop tight.

const ASCII_TO_DIGIT: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    let mut i = b'0';
    while i <= b'9' { t[i as usize] = i - b'0'; i += 1; }
    t
};

/// ASCII byte → decimal digit value. Non-digit bytes map to 255.
/// Alias for `ASCII_TO_DIGIT` — provided as `DIGIT` for conciseness in the
/// integer parsers.
const DIGIT: [u8; 256] = ASCII_TO_DIGIT;

pub trait FromJson<'de>: Sized {
    fn from_json_scanner(scanner: &mut Scanner<'de>) -> Result<Self, Error>;

    fn from_json_str(s: &'de str) -> Result<Self, Error> {
        let mut sc = Scanner::new_str(s);
        Self::from_json_scanner(&mut sc)
    }

    fn from_json_bytes(b: &'de [u8]) -> Result<Self, Error> {
        let mut sc = Scanner::new(b);
        Self::from_json_scanner(&mut sc)
    }
}

// ── &'de str — zero-copy borrow ───────────────────────────────────────────────

impl<'de> FromJson<'de> for &'de str {
    fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
        sc.read_str()?.as_borrowed().ok_or(Error::EscapedString)
    }
}

// ── String ────────────────────────────────────────────────────────────────────

impl<'de> FromJson<'de> for String {
    fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
        sc.read_str().map(|js| js.into_owned())
    }
}

// ── bool ─────────────────────────────────────────────────────────────────────

impl<'de> FromJson<'de> for bool {
    fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
        sc.read_bool()
    }
}

// ── Option<T> ────────────────────────────────────────────────────────────────

impl<'de, T: FromJson<'de>> FromJson<'de> for Option<T> {
    fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
        sc.skip_whitespace();
        if sc.peek_null() { sc.read_null()?; Ok(None) } else { T::from_json_scanner(sc).map(Some) }
    }
}

// ── Vec<T> ───────────────────────────────────────────────────────────────────

impl<'de, T: FromJson<'de>> FromJson<'de> for Vec<T> {
    #[inline]
    fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
        sc.skip_whitespace();
        sc.expect_byte(b'[')?;
        // Fuse whitespace skip + empty-array check in one call.
        if sc.peek_byte_after_ws()? == b']' { sc.advance(); return Ok(Vec::new()); }
        // Pre-allocate with a small guess to avoid the first few reallocs in
        // the common case (canada-style coordinate arrays, object field lists).
        let mut out: Vec<T> = Vec::with_capacity(16);
        loop {
            out.push(T::from_json_scanner(sc)?);
            // Fuse: skip any trailing whitespace then examine the separator —
            // a single call instead of skip_whitespace() + peek_byte().
            match sc.peek_byte_after_ws()? {
                b',' => { sc.advance(); }
                b']' => { sc.advance(); break; }
                _    => return Err(Error::UnexpectedToken),
            }
        }
        Ok(out)
    }
}

// ── u64: explicit overflowing arithmetic for precise overflow detection ───────
//
// Unlike the smaller uint types (which go through u64 + try_from), u64 itself
// must detect overflow without a widening conversion.  We use overflowing_mul /
// overflowing_add so that any arithmetic overflow is caught immediately, then
// check once at the end.

impl<'de> FromJson<'de> for u64 {
    #[inline]
    fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
        sc.skip_whitespace();
        let bytes = sc.read_number_bytes()?;
        let mut n = 0u64;
        let mut overflow = false;
        for &b in bytes {
            let d = DIGIT[b as usize];
            if d == 255 { return Err(Error::InvalidNumber); }
            // Use overflowing_* to detect u64 overflow without a branch per byte.
            let (next, ovf) = n.overflowing_mul(10);
            overflow |= ovf;
            let (next2, ovf2) = next.overflowing_add(d as u64);
            overflow |= ovf2;
            n = next2;
        }
        if overflow { return Err(Error::InvalidNumber); }
        Ok(n)
    }
}

// ── unsigned integers (lookup-table digit accumulation) ──────────────────────
//
// Hot loop: multiply + add with no per-byte branch.  Digit validity is checked
// only once *after* the loop — the 0xFF sentinel propagates naturally into
// an overflow or out-of-range error at the final try_from.
// u64 is handled by the explicit impl above; the macro covers u8/u16/u32/usize.

macro_rules! impl_uint {
    ($($t:ty),*) => {$(
        impl<'de> FromJson<'de> for $t {
            #[inline]
            fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
                sc.skip_whitespace();
                let bytes = sc.read_number_bytes()?;
                let mut n = 0u64;
                let mut bad = false;
                for &b in bytes {
                    let d = ASCII_TO_DIGIT[b as usize];
                    bad |= d == 0xFF;
                    n = n.wrapping_mul(10).wrapping_add(d as u64);
                }
                if bad { return Err(Error::InvalidNumber); }
                <$t>::try_from(n).map_err(|_| Error::InvalidNumber)
            }
        }
    )*};
}

// ── signed integers (lookup-table digit accumulation) ────────────────────────

macro_rules! impl_sint {
    ($($t:ty),*) => {$(
        impl<'de> FromJson<'de> for $t {
            #[inline]
            fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
                sc.skip_whitespace();
                let bytes = sc.read_number_bytes()?;
                let (neg, digits) = if bytes.first() == Some(&b'-') { (true, &bytes[1..]) } else { (false, bytes) };
                let mut n = 0u64;
                let mut bad = false;
                for &b in digits {
                    let d = ASCII_TO_DIGIT[b as usize];
                    bad |= d == 0xFF;
                    n = n.wrapping_mul(10).wrapping_add(d as u64);
                }
                if bad { return Err(Error::InvalidNumber); }
                // Convert to i64 considering sign; rely on try_from for range check.
                let signed: i64 = if neg {
                    // Use wrapping_neg; out-of-range caught by try_from below.
                    (n as i64).wrapping_neg()
                } else {
                    n as i64
                };
                // Extra overflow check: if neg and n > i64::MAX+1 as u64, it wrapped wrong.
                if neg && n > (i64::MAX as u64 + 1) { return Err(Error::InvalidNumber); }
                if !neg && n > i64::MAX as u64 { return Err(Error::InvalidNumber); }
                <$t>::try_from(signed).map_err(|_| Error::InvalidNumber)
            }
        }
    )*};
}

// ── floats ────────────────────────────────────────────────────────────────────
//
// With `fast-float` feature: use fast-float2 crate for ~3× faster parsing.
// Without: fall back to std str::parse.

impl<'de> FromJson<'de> for f64 {
    #[inline]
    fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
        sc.skip_whitespace();
        #[cfg(feature = "fast-float")]
        {
            // Single-pass: parse_partial parses the float AND returns bytes consumed,
            // eliminating the separate forward-scan that read_number_bytes() does.
            let (val, consumed) = fast_float2::parse_partial::<f64, _>(sc.remaining_input())
                .map_err(|_| Error::InvalidNumber)?;
            sc.advance_by(consumed);
            return Ok(val);
        }
        #[cfg(not(feature = "fast-float"))]
        {
            let bytes = sc.read_number_bytes()?;
            let s = core::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
            s.parse::<f64>().map_err(|_| Error::InvalidNumber)
        }
    }
}

impl<'de> FromJson<'de> for f32 {
    #[inline]
    fn from_json_scanner(sc: &mut Scanner<'de>) -> Result<Self, Error> {
        sc.skip_whitespace();
        #[cfg(feature = "fast-float")]
        {
            let (val, consumed) = fast_float2::parse_partial::<f32, _>(sc.remaining_input())
                .map_err(|_| Error::InvalidNumber)?;
            sc.advance_by(consumed);
            return Ok(val);
        }
        #[cfg(not(feature = "fast-float"))]
        {
            let bytes = sc.read_number_bytes()?;
            let s = core::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
            s.parse::<f32>().map_err(|_| Error::InvalidNumber)
        }
    }
}

impl_uint!(u8, u16, u32, usize); // u64 has a hand-written impl above
impl_sint!(i8, i16, i32, i64, isize);

// Note: Vec<f64> automatically benefits from the fast f64::from_json_scanner
// above (fast-float2 when enabled) via the generic Vec<T> implementation.
// Stable Rust does not allow specialization, so no separate Vec<f64> impl is
// possible — the generic impl above (with fused peek_byte_after_ws and
// pre-allocation) is the optimized hot path for nested coordinate arrays.
