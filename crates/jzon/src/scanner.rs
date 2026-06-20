use crate::{simd, Error};

#[cold]
#[inline]
fn err_eof() -> Error { Error::UnexpectedEof }

#[cold]
#[inline]
fn err_token() -> Error { Error::UnexpectedToken }

/// A parsed JSON string: either a zero-copy borrow or a heap-allocated value.
///
/// The `BorrowedNoEsc` variant is returned by [`Scanner::read_str`] when no
/// escape sequences were present in the JSON input.  This lets the serializer
/// skip the `find_escape` scan entirely — the string is provably escape-free.
pub enum JsonStr<'de> {
    /// Zero-copy borrow from the input.  **No longer emitted by [`Scanner::read_str`]**
    /// (use [`JsonStr::BorrowedNoEsc`] instead); kept for API compatibility.
    /// `ToJson` will run `write_escaped_str` on this variant.
    Borrowed(&'de str),
    /// Zero-copy borrow whose content is **provably escape-free** (the scanner
    /// hit a closing `"` before any `\\`).  The serializer can bypass the
    /// `find_escape` scan and write the bytes directly.
    BorrowedNoEsc(&'de str),
    Owned(String),
}

impl<'de> JsonStr<'de> {
    #[inline]
    pub fn as_borrowed(&self) -> Option<&'de str> {
        match self {
            JsonStr::Borrowed(s) => Some(s),
            JsonStr::BorrowedNoEsc(s) => Some(s),
            JsonStr::Owned(_) => None,
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        match self {
            JsonStr::Borrowed(s) => s,
            JsonStr::BorrowedNoEsc(s) => s,
            JsonStr::Owned(s) => s.as_str(),
        }
    }

    #[inline]
    pub fn into_owned(self) -> String {
        match self {
            JsonStr::Borrowed(s) => s.to_owned(),
            JsonStr::BorrowedNoEsc(s) => s.to_owned(),
            JsonStr::Owned(s) => s,
        }
    }
}

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

    #[inline]
    pub fn peek_byte(&self) -> Result<u8, Error> {
        self.input.get(self.pos).copied().ok_or_else(err_eof)
    }

    #[inline]
    pub fn advance(&mut self) {
        self.pos += 1;
    }

    /// Byte offset into the input slice — used by internally-tagged enum parsers to checkpoint and re-scan.
    #[inline]
    pub fn pos(&self) -> usize { self.pos }

    #[inline]
    pub fn set_pos(&mut self, saved_pos: usize) { self.pos = saved_pos; }

    #[inline]
    pub fn advance_by(&mut self, n: usize) {
        self.pos += n;
    }

    /// Remaining unprocessed input — used by single-pass float parsers (`fast_float2::parse_partial`).
    #[inline]
    pub fn remaining_input(&self) -> &'de [u8] {
        &self.input[self.pos..]
    }

    #[inline]
    pub fn expect_byte(&mut self, expected: u8) -> Result<(), Error> {
        match self.input.get(self.pos) {
            Some(&b) if b == expected => { self.pos += 1; Ok(()) }
            _ => Err(err_token()),
        }
    }

    pub fn expect_bytes(&mut self, expected: &[u8]) -> Result<(), Error> {
        let end = self.pos + expected.len();
        if self.input.get(self.pos..end) == Some(expected) {
            self.pos = end;
            Ok(())
        } else {
            Err(err_token())
        }
    }

    #[inline(always)]
    pub fn skip_whitespace(&mut self) {
        // Fast path: compact JSON has no leading whitespace — skip the loop entirely.
        // All structural bytes are > b' ' (32), so this correctly identifies non-whitespace.
        if let Some(&b) = self.input.get(self.pos) {
            if b > b' ' { return; }
        } else {
            return;
        }
        self.skip_whitespace_swar();
    }

    /// SWAR whitespace skipper — called only when the first byte IS whitespace.
    ///
    /// JSON whitespace (ECMA-404 §2) is exactly: 0x09 (TAB), 0x0A (LF),
    /// 0x0D (CR), 0x20 (SP).  VT (0x0B) and FF (0x0C) are NOT valid JSON
    /// whitespace even though they are ≤ 0x20.
    ///
    /// Not `#[cold]` — pretty-printed JSON calls this on every field separator.
    #[inline]
    fn skip_whitespace_swar(&mut self) {
        // 8-byte SWAR bulk scan: all 4 JSON whitespace bytes (0x09,0x0A,0x0D,0x20)
        // are ≤ 0x20, so the "all high-bits set after sub(0x21)" trick bulk-skips
        // them. VT(0x0B) and FF(0x0C) also satisfy this, so the byte-by-byte tail
        // re-validates: bulk skip advances past any ≤0x20 byte, tail rejects non-WS.
        while self.pos + 8 <= self.input.len() {
            let chunk = u64::from_le_bytes(
                self.input[self.pos..self.pos + 8].try_into().unwrap(),
            );
            let sub = chunk.wrapping_sub(0x2121_2121_2121_2121_u64);
            if (sub & 0x8080_8080_8080_8080_u64) == 0x8080_8080_8080_8080_u64 {
                self.pos += 8;
            } else {
                break;
            }
        }
        // Byte-by-byte tail: precise WS test rejects VT/FF.
        while let Some(&b) = self.input.get(self.pos) {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    #[inline(always)]
    pub fn peek_byte_after_ws(&mut self) -> Result<u8, Error> {
        self.skip_whitespace();
        self.peek_byte()
    }

    /// After parsing a top-level value, skip trailing whitespace and verify
    /// that no non-whitespace bytes remain (ECMA-404 requires a single value).
    #[inline]
    pub fn expect_eof(&mut self) -> Result<(), Error> {
        self.skip_whitespace();
        if self.pos < self.input.len() {
            Err(Error::UnexpectedToken)
        } else {
            Ok(())
        }
    }

    /// Read a JSON object key as a zero-copy `&'de [u8]`.
    /// Returns `Error::EscapedKey` if the key contains backslashes.
    pub fn read_key(&mut self) -> Result<&'de [u8], Error> {
        self.skip_whitespace();
        self.expect_byte(b'"')?;
        let start = self.pos;
        // When simd-intrinsics are active the find_escape kernels (NEON/AVX2)
        // fuse quote+backslash+ctrl<0x20 in a single SIMD pass — negligible extra
        // cost over find(). On scalar/SWAR paths, use find() + SWAR has_control_char
        // to avoid a byte-by-byte second pass.
        #[cfg(feature = "simd-intrinsics")]
        let stop = simd::find_escape(self.input, self.pos);
        #[cfg(not(feature = "simd-intrinsics"))]
        let stop = simd::find(self.input, self.pos);

        match self.input.get(stop) {
            Some(&b'"') => {
                #[cfg(not(feature = "simd-intrinsics"))]
                if simd::has_control_char(&self.input[start..stop]) {
                    return Err(Error::InvalidEscape);
                }
                self.pos = stop + 1;
                Ok(&self.input[start..stop])
            }
            Some(&b'\\') => Err(Error::EscapedKey),
            Some(_) => Err(Error::InvalidEscape), // control char from find_escape
            _ => Err(err_eof()),
        }
    }

    /// Read a JSON object key and the mandatory `:` separator in one call.
    #[inline]
    pub fn read_key_colon(&mut self) -> Result<&'de [u8], Error> {
        let key = self.read_key()?;
        // Fast path: ':' almost always immediately follows the closing '"' in compact JSON.
        if self.input.get(self.pos) == Some(&b':') {
            self.pos += 1;
        } else {
            self.skip_whitespace();
            self.expect_byte(b':')?;
        }
        Ok(key)
    }

    /// Read a JSON object key and the mandatory `:` separator, returning a
    /// validated UTF-8 key borrow.
    #[inline]
    pub fn read_str_key_colon(&mut self) -> Result<&'de str, Error> {
        let key = self.read_key_colon()?;
        core::str::from_utf8(key).map_err(|_| Error::InvalidUtf8)
    }

    /// Read a JSON string value.
    ///
    /// Returns [`JsonStr::BorrowedNoEsc`] when no escape sequences are present
    /// (zero allocation, provably escape-free), or [`JsonStr::Owned`] after
    /// unescaping.
    pub fn read_str(&mut self) -> Result<JsonStr<'de>, Error> {
        self.skip_whitespace();
        self.expect_byte(b'"')?;
        let start = self.pos;
        #[cfg(feature = "simd-intrinsics")]
        let stop = simd::find_escape(self.input, start);
        #[cfg(not(feature = "simd-intrinsics"))]
        let stop = simd::find(self.input, start);

        match self.input.get(stop) {
            Some(&b'"') => {
                #[cfg(not(feature = "simd-intrinsics"))]
                if simd::has_control_char(&self.input[start..stop]) {
                    return Err(Error::InvalidEscape);
                }
                let s = core::str::from_utf8(&self.input[start..stop])
                    .map_err(|_| Error::InvalidUtf8)?;
                self.pos = stop + 1;

                #[cfg(feature = "stats")]
                { self.stats.zero_copy_borrows += 1; }

                Ok(JsonStr::BorrowedNoEsc(s))
            }
            Some(&b'\\') => {
                self.pos = stop;
                let owned = self.unescape_from(start)?;

                #[cfg(feature = "stats")]
                { self.stats.heap_allocations += 1; }

                Ok(JsonStr::Owned(owned))
            }
            Some(_) => Err(Error::InvalidEscape), // control char < 0x20
            None => Err(err_eof()),
        }
    }

    #[inline]
    fn scan_ascii_digits(&mut self) -> usize {
        let start = self.pos;

        // SWAR digit scan: for byte b, b is b'0'..=b'9' iff (b - 0x30) is 0..=9.
        // Two conditions: (1) sub has no high bits (rules out bytes >= 0xB0),
        // (2) sub + 0x76 has no high bits (rules out sub bytes 10..=0x7F).
        #[inline(always)]
        fn swar_all_digits(chunk: u64) -> bool {
            let sub = chunk.wrapping_sub(0x3030_3030_3030_3030_u64);
            if (sub & 0x8080_8080_8080_8080_u64) != 0 { return false; }
            let check = sub.wrapping_add(0x7676_7676_7676_7676_u64);
            (check & 0x8080_8080_8080_8080_u64) == 0
        }

        while self.pos + 8 <= self.input.len() {
            let chunk = u64::from_le_bytes(
                self.input[self.pos..self.pos + 8].try_into().unwrap(),
            );
            if swar_all_digits(chunk) { self.pos += 8; } else { break; }
        }
        while let Some(&b) = self.input.get(self.pos) {
            if b.is_ascii_digit() { self.pos += 1; } else { break; }
        }

        self.pos - start
    }

    /// Scan a JSON number and return the raw byte slice (zero-copy).
    pub fn read_number_bytes(&mut self) -> Result<&'de [u8], Error> {
        self.skip_whitespace();
        let start = self.pos;
        if self.input.get(self.pos) == Some(&b'-') { self.pos += 1; }

        // Read the integer part.  If it starts with '0', the spec forbids any
        // further digit immediately following (leading zeros like "01" are invalid).
        match self.input.get(self.pos) {
            Some(&b'0') => {
                self.pos += 1;
                // Leading zero: next byte must NOT be another digit.
                if matches!(self.input.get(self.pos), Some(b'0'..=b'9')) {
                    return Err(Error::InvalidNumber);
                }
            }
            Some(&(b'1'..=b'9')) => {
                self.pos += 1;
                self.scan_ascii_digits();
            }
            _ => {} // will be caught by the end-check below
        }

        if self.input.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            // At least one digit must follow the decimal point.
            if self.scan_ascii_digits() == 0 {
                // No digit after '.': "1." is invalid JSON.
                return Err(Error::InvalidNumber);
            }
        }
        if matches!(self.input.get(self.pos), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.input.get(self.pos), Some(b'+') | Some(b'-')) { self.pos += 1; }
            if self.scan_ascii_digits() == 0 {
                return Err(Error::InvalidNumber);
            }
        }
        let end = self.pos;
        if end == start || (end == start + 1 && self.input[start] == b'-') {
            return Err(Error::InvalidNumber);
        }

        #[cfg(feature = "stats")]
        { self.stats.bytes_scanned += (end - start) as u64; }

        Ok(&self.input[start..end])
    }

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
                self.pos += 4;
                if self.input.get(self.pos - 3..self.pos) == Some(b"rue") {
                    Ok(true)
                } else {
                    self.pos -= 4;
                    Err(err_token())
                }
            }
            Some(&b'f') => {
                self.pos += 5;
                if self.input.get(self.pos - 4..self.pos) == Some(b"alse") {
                    Ok(false)
                } else {
                    self.pos -= 5;
                    Err(err_token())
                }
            }
            _ => Err(err_token()),
        }
    }

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
            _                 => Err(err_token()),
        }
    }

    fn skip_string(&mut self) -> Result<(), Error> {
        self.expect_byte(b'"')?;
        loop {
            match self.input.get(self.pos) {
                Some(&b'"')  => { self.pos += 1; return Ok(()); }
                Some(&b'\\') => { self.pos += 2; }
                Some(_)      => { self.pos += 1; }
                None         => return Err(err_eof()),
            }
        }
    }

    /// Skip remaining array elements and the closing `]`.
    /// Call this after a partial `SeqAccess` visit to drain any unconsumed
    /// elements so the scanner is positioned after the `]`.
    pub fn skip_array_tail(&mut self) -> Result<(), Error> {
        loop {
            self.skip_whitespace();
            match self.peek_byte()? {
                b']' => { self.pos += 1; return Ok(()); }
                b',' => { self.pos += 1; self.skip_value()?; }
                _    => { self.skip_value()?; }
            }
        }
    }

    /// Skip remaining fields of an already-opened object (cursor is just past `{`).
    /// Used by internally-tagged enum deserialization when the variant is unknown.
    pub fn skip_object_tail(&mut self) -> Result<(), Error> {
        loop {
            self.skip_whitespace();
            match self.peek_byte()? {
                b'}' => { self.pos += 1; return Ok(()); }
                b'"' => {
                    self.skip_string()?;
                    self.skip_whitespace();
                    self.expect_byte(b':')?;
                    self.skip_value()?;
                    self.skip_whitespace();
                    match self.peek_byte()? {
                        b',' => { self.pos += 1; }
                        b'}' => { self.pos += 1; return Ok(()); }
                        _ => return Err(err_token()),
                    }
                }
                _ => return Err(err_token()),
            }
        }
    }

    fn skip_object(&mut self) -> Result<(), Error> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();
        if self.input.get(self.pos) == Some(&b'}') { self.pos += 1; return Ok(()); }
        loop {
            self.skip_string()?;
            self.skip_whitespace();
            self.expect_byte(b':')?;
            self.skip_value()?;
            self.skip_whitespace();
            match self.peek_byte()? {
                b',' => { self.pos += 1; self.skip_whitespace(); }
                b'}' => { self.pos += 1; break; }
                _    => return Err(err_token()),
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
                _    => return Err(err_token()),
            }
        }
        Ok(())
    }

    /// Unescape a JSON string whose content starts at `content_start` and whose
    /// first backslash is at `self.pos`.  Returns the fully decoded `String`.
    fn unescape_from(&mut self, content_start: usize) -> Result<String, Error> {
        // Output is at most as long as the remaining input — preallocating
        // avoids the Vec-doubling realloc chain on escape-heavy strings.
        let mut buf: Vec<u8> =
            Vec::with_capacity(self.input.len().saturating_sub(content_start));
        // The caller (read_str) already positioned self.pos at the first `\`;
        // find_escape already verified no control chars before that point,
        // so the prefix is clean and we copy it directly.
        buf.extend_from_slice(&self.input[content_start..self.pos]);

        loop {
            match self.input.get(self.pos) {
                Some(&b'"') => { self.pos += 1; break; }
                Some(&b'\\') => {
                    self.pos += 1;
                    let esc = self.input.get(self.pos).copied().ok_or_else(err_eof)?;
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
                            let c = if (0xD800..=0xDBFF).contains(&code) {
                                self.pos += 4;
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
                            continue;
                        }
                        _ => return Err(Error::InvalidEscape),
                    }
                }
                Some(_) => {
                    let seg_start = self.pos;
                    // Single pass: find_escape stops at `"`, `\`, or any byte < 0x20.
                    let stop = simd::find_escape(self.input, self.pos);
                    match self.input.get(stop) {
                        Some(&b'"') | Some(&b'\\') => {
                            buf.extend_from_slice(&self.input[seg_start..stop]);
                            self.pos = stop;
                        }
                        Some(_) => return Err(Error::InvalidEscape), // control char
                        None => return Err(err_eof()),
                    }
                }
                None => return Err(err_eof()),
            }
        }

        String::from_utf8(buf).map_err(|_| Error::InvalidUtf8)
    }
}
