//! Zero-copy JSON scanner — the low-level cursor consumed by generated `FromJson` impls.
//!
//! ## Zero-copy guarantee
//!
//! The scanner holds a `&'de [u8]` borrow of the entire input.  When
//! `read_str()` is called and the string contains no escape sequences, it
//! returns `JsonStr::Borrowed(&'de str)` — a slice into the *original* input
//! with no allocation.  Only when escape sequences are present does it
//! allocate via `JsonStr::Owned(String)`.
//!
//! ## SIMD scanning
//!
//! The inner hot loop that searches for `"` or `\` is delegated to
//! `crate::simd::find()`, which selects the widest safe implementation
//! available:
//!
//! * Default: SWAR u64 (8 bytes/iter, zero deps, all platforms).
//! * `simd` feature: u128 SWAR (16 bytes/iter).
//! * `simd + unstable` features: `std::simd` portable SIMD (32–64 bytes/iter).
//!
//! All paths are **safe Rust** — no `unsafe` blocks anywhere.

use crate::{simd, Error};

// ── JsonStr ───────────────────────────────────────────────────────────────────

/// A parsed JSON string: either a zero-copy borrow or a heap-allocated value.
pub enum JsonStr<'de> {
    Borrowed(&'de str),
    Owned(String),
}

impl<'de> JsonStr<'de> {
    /// Returns the borrowed slice if no unescaping was performed.
    #[inline]
    pub fn as_borrowed(&self) -> Option<&'de str> {
        match self {
            JsonStr::Borrowed(s) => Some(s),
            JsonStr::Owned(_) => None,
        }
    }

    /// Borrow the contained string without cloning.
    #[inline]
    pub fn as_str(&self) -> &str {
        match self {
            JsonStr::Borrowed(s) => s,
            JsonStr::Owned(s) => s.as_str(),
        }
    }

    /// Convert to an owned `String`, cloning only when needed.
    #[inline]
    pub fn into_owned(self) -> String {
        match self {
            JsonStr::Borrowed(s) => s.to_owned(),
            JsonStr::Owned(s) => s,
        }
    }
}

// ── Scanner ───────────────────────────────────────────────────────────────────

pub struct Scanner<'de> {
    input: &'de [u8],
    pos: usize,
    #[cfg(feature = "stats")]
    pub stats: crate::stats::ScannerStats,
}

impl<'de> Scanner<'de> {
    #[inline]
    pub fn new(input: &'de [u8]) -> Self {
        Scanner {
            input,
            pos: 0,
            #[cfg(feature = "stats")]
            stats: crate::stats::ScannerStats::default(),
        }
    }

    #[inline]
    pub fn new_str(s: &'de str) -> Self {
        Self::new(s.as_bytes())
    }

    // ── cursor primitives ─────────────────────────────────────────────────────

    #[inline]
    pub fn peek_byte(&self) -> Result<u8, Error> {
        self.input.get(self.pos).copied().ok_or(Error::UnexpectedEof)
    }

    #[inline]
    pub fn advance(&mut self) {
        self.pos += 1;
    }

    /// Advance the cursor by `n` bytes (unchecked — caller must ensure `n <= remaining`).
    #[inline]
    pub fn advance_by(&mut self, n: usize) {
        self.pos += n;
    }

    /// The remaining unprocessed input starting at the current cursor position.
    /// Used by single-pass float parsers (`fast_float2::parse_partial`).
    #[inline]
    pub fn remaining_input(&self) -> &'de [u8] {
        &self.input[self.pos..]
    }

    #[inline]
    pub fn expect_byte(&mut self, expected: u8) -> Result<(), Error> {
        match self.input.get(self.pos) {
            Some(&b) if b == expected => { self.pos += 1; Ok(()) }
            _ => Err(Error::UnexpectedToken),
        }
    }

    pub fn expect_bytes(&mut self, expected: &[u8]) -> Result<(), Error> {
        let end = self.pos + expected.len();
        if self.input.get(self.pos..end) == Some(expected) {
            self.pos = end;
            Ok(())
        } else {
            Err(Error::UnexpectedToken)
        }
    }

    #[inline(always)]
    pub fn skip_whitespace(&mut self) {
        // Fast path: compact JSON (produced by our serializer) has no leading
        // whitespace.  A single peek avoids the loop entirely in the common case.
        // All structural JSON characters have byte values > b' ' (32), so the
        // check `b > b' '` correctly identifies any non-whitespace ASCII byte.
        if let Some(&b) = self.input.get(self.pos) {
            if b > b' ' { return; }
        } else {
            return;
        }
        // Slow path: actual whitespace present — scan forward.
        while let Some(&b) = self.input.get(self.pos) {
            if b > b' ' { break; }
            self.pos += 1;
        }
    }

    /// Skip whitespace then peek at the next byte — fused for hot-path use.
    #[inline]
    pub fn peek_byte_after_ws(&mut self) -> Result<u8, Error> {
        self.skip_whitespace();
        self.peek_byte()
    }

    // ── string reading ────────────────────────────────────────────────────────

    /// Read a JSON object key as a raw `&'de [u8]` byte slice (zero-copy).
    /// Returns `Error::EscapedKey` if the key contains backslashes (rare).
    ///
    /// Uses the SWAR/portable-SIMD `simd::find()` scanner (8-byte chunks) to
    /// locate the closing `"` or a `\` escape, replacing the previous per-byte
    /// loop.  This gives 3–8× faster key scanning on typical short field names.
    pub fn read_key(&mut self) -> Result<&'de [u8], Error> {
        self.skip_whitespace();
        self.expect_byte(b'"')?;
        let start = self.pos;
        // Use simd::find (SWAR/portable-SIMD) to locate closing `"` or `\`.
        let stop = simd::find(self.input, self.pos);
        match self.input.get(stop) {
            Some(&b'"') => {
                let k = &self.input[start..stop];
                self.pos = stop + 1;
                Ok(k)
            }
            Some(&b'\\') => Err(Error::EscapedKey),
            _ => Err(Error::UnexpectedEof),
        }
    }

    /// Read a JSON object key and the mandatory `:` separator in one call.
    /// Skips leading whitespace before the key. Returns the key bytes.
    ///
    /// This fuses `read_key()` + `skip_whitespace()` + `expect_byte(b':')` into
    /// a single method.  On the fast path (compact JSON where `:` immediately
    /// follows the closing `"`) it avoids two function-call overheads and one
    /// whitespace scan.
    #[inline]
    pub fn read_key_colon(&mut self) -> Result<&'de [u8], Error> {
        let key = self.read_key()?;
        // Fast path: ':' almost always immediately follows the closing '"'
        // (no whitespace in compact JSON).
        if self.input.get(self.pos) == Some(&b':') {
            self.pos += 1;
        } else {
            self.skip_whitespace();
            self.expect_byte(b':')?;
        }
        Ok(key)
    }

    /// Read a JSON string value.
    ///
    /// Returns `Borrowed(&'de str)` when no escape sequences are present
    /// (the common case, zero allocation), or `Owned(String)` after unescaping.
    pub fn read_str(&mut self) -> Result<JsonStr<'de>, Error> {
        self.skip_whitespace();
        self.expect_byte(b'"')?;
        let start = self.pos;

        // Use the best available SIMD/SWAR routine to find the end of the string.
        let stop = simd::find(self.input, start);

        match self.input.get(stop) {
            Some(&b'"') => {
                // Fast path — no escapes, zero-copy borrow.
                let s = core::str::from_utf8(&self.input[start..stop])
                    .map_err(|_| Error::InvalidUtf8)?;
                self.pos = stop + 1;

                #[cfg(feature = "stats")]
                { self.stats.zero_copy_borrows += 1; }

                Ok(JsonStr::Borrowed(s))
            }
            Some(&b'\\') => {
                // Slow path — has escapes, unescape into a heap-allocated String.
                self.pos = stop;
                let owned = self.unescape_from(start)?;

                #[cfg(feature = "stats")]
                { self.stats.heap_allocations += 1; }

                Ok(JsonStr::Owned(owned))
            }
            _ => Err(Error::UnexpectedEof),
        }
    }

    // ── number reading ────────────────────────────────────────────────────────

    /// Scan a JSON number and return the raw byte slice (zero-copy).
    pub fn read_number_bytes(&mut self) -> Result<&'de [u8], Error> {
        self.skip_whitespace();
        let start = self.pos;
        if self.input.get(self.pos) == Some(&b'-') { self.pos += 1; }
        while let Some(&b) = self.input.get(self.pos) { if b.is_ascii_digit() { self.pos += 1; } else { break; } }
        if self.input.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            while let Some(&b) = self.input.get(self.pos) { if b.is_ascii_digit() { self.pos += 1; } else { break; } }
        }
        if matches!(self.input.get(self.pos), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.input.get(self.pos), Some(b'+') | Some(b'-')) { self.pos += 1; }
            while let Some(&b) = self.input.get(self.pos) { if b.is_ascii_digit() { self.pos += 1; } else { break; } }
        }
        let end = self.pos;
        if end == start || (end == start + 1 && self.input[start] == b'-') {
            return Err(Error::InvalidNumber);
        }

        #[cfg(feature = "stats")]
        { self.stats.bytes_scanned += (end - start) as u64; }

        Ok(&self.input[start..end])
    }

    // ── null / bool ───────────────────────────────────────────────────────────

    /// Returns true if the next (non-whitespace) bytes are `null` — does NOT consume.
    #[inline]
    pub fn peek_null(&mut self) -> bool {
        self.skip_whitespace();
        self.input.get(self.pos..self.pos + 4) == Some(b"null")
    }

    pub fn read_null(&mut self) -> Result<(), Error> {
        self.skip_whitespace();
        self.expect_bytes(b"null")
    }

    pub fn read_bool(&mut self) -> Result<bool, Error> {
        self.skip_whitespace();
        match self.input.get(self.pos) {
            Some(&b't') => {
                self.pos += 4; // advance past 't','r','u','e'
                // Validate we actually read "true" — check the 3 chars we skipped.
                if self.input.get(self.pos - 3..self.pos) == Some(b"rue") {
                    Ok(true)
                } else {
                    self.pos -= 4;
                    Err(Error::UnexpectedToken)
                }
            }
            Some(&b'f') => {
                self.pos += 5; // advance past 'f','a','l','s','e'
                if self.input.get(self.pos - 4..self.pos) == Some(b"alse") {
                    Ok(false)
                } else {
                    self.pos -= 5;
                    Err(Error::UnexpectedToken)
                }
            }
            _ => Err(Error::UnexpectedToken),
        }
    }

    // ── skip ─────────────────────────────────────────────────────────────────

    /// Skip over any JSON value — used for unknown fields.
    pub fn skip_value(&mut self) -> Result<(), Error> {
        self.skip_whitespace();
        match self.peek_byte()? {
            b'"'              => self.skip_string(),
            b'{'              => self.skip_object(),
            b'['              => self.skip_array(),
            b't'              => self.expect_bytes(b"true"),
            b'f'              => self.expect_bytes(b"false"),
            b'n'              => self.expect_bytes(b"null"),
            b'-' | b'0'..=b'9' => { self.read_number_bytes()?; Ok(()) }
            _                 => Err(Error::UnexpectedToken),
        }
    }

    fn skip_string(&mut self) -> Result<(), Error> {
        self.expect_byte(b'"')?;
        loop {
            match self.input.get(self.pos) {
                Some(&b'"')  => { self.pos += 1; return Ok(()); }
                Some(&b'\\') => { self.pos += 2; }
                Some(_)      => { self.pos += 1; }
                None         => return Err(Error::UnexpectedEof),
            }
        }
    }

    fn skip_object(&mut self) -> Result<(), Error> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();
        if self.input.get(self.pos) == Some(&b'}') { self.pos += 1; return Ok(()); }
        loop {
            self.skip_string()?;  // key
            self.skip_whitespace();
            self.expect_byte(b':')?;
            self.skip_value()?;
            self.skip_whitespace();
            match self.peek_byte()? {
                b',' => { self.pos += 1; self.skip_whitespace(); }
                b'}' => { self.pos += 1; break; }
                _    => return Err(Error::UnexpectedToken),
            }
        }
        Ok(())
    }

    fn skip_array(&mut self) -> Result<(), Error> {
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        if self.input.get(self.pos) == Some(&b']') { self.pos += 1; return Ok(()); }
        loop {
            self.skip_value()?;
            self.skip_whitespace();
            match self.peek_byte()? {
                b',' => { self.pos += 1; self.skip_whitespace(); }
                b']' => { self.pos += 1; break; }
                _    => return Err(Error::UnexpectedToken),
            }
        }
        Ok(())
    }

    // ── unescape helper ───────────────────────────────────────────────────────

    /// Unescape a JSON string whose content starts at `content_start` and whose
    /// first backslash is at `self.pos`.  Returns the fully decoded `String`.
    fn unescape_from(&mut self, content_start: usize) -> Result<String, Error> {
        let mut buf: Vec<u8> = self.input[content_start..self.pos].to_vec();

        loop {
            match self.input.get(self.pos) {
                Some(&b'"') => { self.pos += 1; break; }
                Some(&b'\\') => {
                    self.pos += 1;
                    let esc = self.input.get(self.pos).copied().ok_or(Error::UnexpectedEof)?;
                    self.pos += 1;
                    match esc {
                        b'"'  => buf.push(b'"'),
                        b'\\' => buf.push(b'\\'),
                        b'/'  => buf.push(b'/'),
                        b'n'  => buf.push(b'\n'),
                        b't'  => buf.push(b'\t'),
                        b'r'  => buf.push(b'\r'),
                        b'b'  => buf.push(0x08),
                        b'f'  => buf.push(0x0C),
                        b'u'  => {
                            let hex = self.input.get(self.pos..self.pos + 4).ok_or(Error::InvalidEscape)?;
                            let s = core::str::from_utf8(hex).map_err(|_| Error::InvalidEscape)?;
                            let code = u32::from_str_radix(s, 16).map_err(|_| Error::InvalidEscape)?;
                            // Handle surrogate pairs (high surrogate → look for low surrogate).
                            let c = if (0xD800..=0xDBFF).contains(&code) {
                                self.pos += 4;
                                // Expect \uXXXX for the low surrogate.
                                if self.input.get(self.pos..self.pos + 2) != Some(b"\\u") {
                                    return Err(Error::InvalidEscape);
                                }
                                self.pos += 2;
                                let lo_hex = self.input.get(self.pos..self.pos + 4).ok_or(Error::InvalidEscape)?;
                                let lo_s = core::str::from_utf8(lo_hex).map_err(|_| Error::InvalidEscape)?;
                                let lo = u32::from_str_radix(lo_s, 16).map_err(|_| Error::InvalidEscape)?;
                                self.pos += 4;
                                let combined = 0x10000 + ((code - 0xD800) << 10) + (lo - 0xDC00);
                                char::from_u32(combined).ok_or(Error::InvalidEscape)?
                            } else {
                                self.pos += 4;
                                char::from_u32(code).ok_or(Error::InvalidEscape)?
                            };
                            let mut tmp = [0u8; 4];
                            buf.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
                            continue; // pos already advanced
                        }
                        _ => return Err(Error::InvalidEscape),
                    }
                }
                Some(_) => {
                    // Bulk-copy non-special bytes using SWAR/SIMD find.
                    let seg_start = self.pos;
                    let stop = simd::find(self.input, self.pos);
                    buf.extend_from_slice(&self.input[seg_start..stop]);
                    self.pos = stop;
                }
                None => return Err(Error::UnexpectedEof),
            }
        }

        String::from_utf8(buf).map_err(|_| Error::InvalidUtf8)
    }
}
