//! `jzon_serde` — serde-compatible JSON serializer/deserializer backed by jzon's
//! SIMD string escaping and zero-copy scanner.
//!
//! Works with **any** type that derives `serde::Serialize` / `serde::Deserialize`.
//!
//! # Usage
//!
//! ```rust,ignore
//! use jzon_serde::{to_string, from_str};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug, PartialEq)]
//! struct Point { x: f64, y: f64 }
//!
//! let p = Point { x: 1.0, y: 2.0 };
//! let json = to_string(&p).unwrap();
//! let p2: Point = from_str(&json).unwrap();
//! assert_eq!(p, p2);
//! ```

use serde::ser::{
    self as ser_trait, Serialize, SerializeMap, SerializeSeq, SerializeStruct,
    SerializeStructVariant, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
};
use serde::de::{self as de_trait, DeserializeOwned, Visitor, MapAccess, SeqAccess, EnumAccess, VariantAccess};

use jzon::ser::{write_escaped_str, write_u64, write_i64, ToJson};
use jzon::{Scanner, JsonStr};

#[derive(Debug)]
pub enum Error {
    Custom(String),
    InvalidUtf8,
    Io(std::io::Error),
    Scanner(jzon::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Custom(m)    => write!(f, "{m}"),
            Error::InvalidUtf8  => write!(f, "invalid UTF-8"),
            Error::Io(e)        => write!(f, "I/O error: {e}"),
            Error::Scanner(e)   => write!(f, "JSON parse error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e)      => Some(e),
            Error::Scanner(e) => Some(e),
            _                 => None,
        }
    }
}

impl ser_trait::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl de_trait::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl From<jzon::Error> for Error {
    fn from(e: jzon::Error) -> Self {
        Error::Scanner(e)
    }
}

// ── float helpers ─────────────────────────────────────────────────────────────
// serde_json always emits a decimal point for whole-number floats (e.g. 3.0 →
// "3.0").  jzon's core ToJson impl omits it when using ryu (fast-float feature)
// to keep the core crate allocation-free.  We add it back here in the serde
// layer so that jzon_serde output matches serde_json expectations.

#[inline]
fn ensure_decimal_point(buf_start: usize, w: &mut Vec<u8>) {
    let written = &w[buf_start..];
    let has_dot = written.contains(&b'.');
    let has_exp = written.contains(&b'e') || written.contains(&b'E');
    if !has_dot && !has_exp {
        w.extend_from_slice(b".0");
    }
}

#[inline]
fn serialize_float64(v: f64, w: &mut Vec<u8>) {
    if !v.is_finite() {
        w.extend_from_slice(b"null");
        return;
    }
    let start = w.len();
    v.json_write(w);
    ensure_decimal_point(start, w);
}

#[inline]
fn serialize_float32(v: f32, w: &mut Vec<u8>) {
    if !v.is_finite() {
        w.extend_from_slice(b"null");
        return;
    }
    let start = w.len();
    v.json_write(w);
    ensure_decimal_point(start, w);
}

pub struct Serializer {
    output: Vec<u8>,
}

pub fn to_string<T: Serialize>(v: &T) -> Result<String, Error> {
    let mut ser = Serializer { output: Vec::with_capacity(128) };
    v.serialize(&mut ser)?;
    String::from_utf8(ser.output).map_err(|_| Error::InvalidUtf8)
}

pub fn to_bytes<T: Serialize>(v: &T) -> Result<Vec<u8>, Error> {
    let mut ser = Serializer { output: Vec::with_capacity(128) };
    v.serialize(&mut ser)?;
    Ok(ser.output)
}

pub fn to_writer<W: std::io::Write, T: Serialize>(mut w: W, v: &T) -> Result<(), Error> {
    let bytes = to_bytes(v)?;
    w.write_all(&bytes).map_err(Error::Io)
}

impl<'a> ser_trait::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    type SerializeSeq            = SeqSerializer<'a>;
    type SerializeTuple          = SeqSerializer<'a>;
    type SerializeTupleStruct    = SeqSerializer<'a>;
    type SerializeTupleVariant   = SeqSerializer<'a>;
    type SerializeMap            = MapSerializer<'a>;
    type SerializeStruct         = MapSerializer<'a>;
    type SerializeStructVariant  = MapSerializer<'a>;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.output.extend_from_slice(if v { b"true" } else { b"false" });
        Ok(())
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<(), Error> { write_i64(v as i64, &mut self.output); Ok(()) }
    #[inline]
    fn serialize_i16(self, v: i16) -> Result<(), Error> { write_i64(v as i64, &mut self.output); Ok(()) }
    #[inline]
    fn serialize_i32(self, v: i32) -> Result<(), Error> { write_i64(v as i64, &mut self.output); Ok(()) }
    #[inline]
    fn serialize_i64(self, v: i64) -> Result<(), Error> { write_i64(v, &mut self.output); Ok(()) }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<(), Error> { write_u64(v as u64, &mut self.output); Ok(()) }
    #[inline]
    fn serialize_u16(self, v: u16) -> Result<(), Error> { write_u64(v as u64, &mut self.output); Ok(()) }
    #[inline]
    fn serialize_u32(self, v: u32) -> Result<(), Error> { write_u64(v as u64, &mut self.output); Ok(()) }
    #[inline]
    fn serialize_u64(self, v: u64) -> Result<(), Error> { write_u64(v, &mut self.output); Ok(()) }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        serialize_float32(v, &mut self.output);
        Ok(())
    }
    #[inline]
    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        serialize_float64(v, &mut self.output);
        Ok(())
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<(), Error> {
        let mut buf = [0u8; 4];
        let s = v.encode_utf8(&mut buf);
        write_escaped_str(s, &mut self.output);
        Ok(())
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<(), Error> {
        write_escaped_str(v, &mut self.output);
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.output.push(b'[');
        for (i, &b) in v.iter().enumerate() {
            if i > 0 { self.output.push(b','); }
            write_u64(b as u64, &mut self.output);
        }
        self.output.push(b']');
        Ok(())
    }

    #[inline]
    fn serialize_none(self) -> Result<(), Error> {
        self.output.extend_from_slice(b"null");
        Ok(())
    }

    #[inline]
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<(), Error> {
        v.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<(), Error> {
        self.output.extend_from_slice(b"null");
        Ok(())
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        self.output.extend_from_slice(b"null");
        Ok(())
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        write_escaped_str(variant, &mut self.output);
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.output.push(b'{');
        write_escaped_str(variant, &mut self.output);
        self.output.push(b':');
        value.serialize(&mut *self)?;
        self.output.push(b'}');
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<SeqSerializer<'a>, Error> {
        self.output.push(b'[');
        Ok(SeqSerializer { ser: self, first: true })
    }

    fn serialize_tuple(self, len: usize) -> Result<SeqSerializer<'a>, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<SeqSerializer<'a>, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<SeqSerializer<'a>, Error> {
        self.output.push(b'{');
        write_escaped_str(variant, &mut self.output);
        self.output.push(b':');
        self.output.push(b'[');
        Ok(SeqSerializer { ser: self, first: true })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<MapSerializer<'a>, Error> {
        self.output.push(b'{');
        Ok(MapSerializer { ser: self, first: true, variant_wrap: false })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<MapSerializer<'a>, Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<MapSerializer<'a>, Error> {
        self.output.push(b'{');
        write_escaped_str(variant, &mut self.output);
        self.output.push(b':');
        self.output.push(b'{');
        Ok(MapSerializer { ser: self, first: true, variant_wrap: true })
    }

    fn is_human_readable(&self) -> bool { true }
}

pub struct SeqSerializer<'a> {
    ser: &'a mut Serializer,
    first: bool,
}

impl<'a> SerializeSeq for SeqSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        if !self.first { self.ser.output.push(b','); }
        self.first = false;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), Error> {
        self.ser.output.push(b']');
        Ok(())
    }
}

macro_rules! delegate_to_seq {
    ($trait:ident, $method:ident) => {
        impl<'a> $trait for SeqSerializer<'a> {
            type Ok = ();
            type Error = Error;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
                SerializeSeq::serialize_element(self, value)
            }
            fn end(self) -> Result<(), Error> { SerializeSeq::end(self) }
        }
    };
}
delegate_to_seq!(SerializeTuple, serialize_element);
delegate_to_seq!(SerializeTupleStruct, serialize_field);

impl<'a> SerializeTupleVariant for SeqSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<(), Error> {
        self.ser.output.push(b']');
        self.ser.output.push(b'}');
        Ok(())
    }
}

pub struct MapSerializer<'a> {
    ser: &'a mut Serializer,
    first: bool,
    /// True when this is a struct-variant that needs an extra closing `}`.
    variant_wrap: bool,
}

impl<'a> SerializeMap for MapSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        if !self.first { self.ser.output.push(b','); }
        self.first = false;
        key.serialize(&mut *self.ser)?;
        self.ser.output.push(b':');
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), Error> {
        self.ser.output.push(b'}');
        if self.variant_wrap { self.ser.output.push(b'}'); }
        Ok(())
    }
}

impl<'a> SerializeStruct for MapSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        if !self.first { self.ser.output.push(b','); }
        self.first = false;
        write_escaped_str(key, &mut self.ser.output);
        self.ser.output.push(b':');
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), Error> {
        self.ser.output.push(b'}');
        if self.variant_wrap { self.ser.output.push(b'}'); }
        Ok(())
    }
}

impl<'a> SerializeStructVariant for MapSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        SerializeStruct::serialize_field(self, key, value)
    }

    fn end(self) -> Result<(), Error> {
        SerializeStruct::end(self)
    }
}

pub struct Deserializer<'de> {
    scanner: Scanner<'de>,
}

pub fn from_str<'de, T: serde::Deserialize<'de>>(s: &'de str) -> Result<T, Error> {
    let scanner = Scanner::new_str(s);
    let mut de = Deserializer { scanner };
    T::deserialize(&mut de)
}

pub fn from_slice<'de, T: serde::Deserialize<'de>>(b: &'de [u8]) -> Result<T, Error> {
    let scanner = Scanner::new(b);
    let mut de = Deserializer { scanner };
    T::deserialize(&mut de)
}

fn from_reader_inner<'de, T: serde::Deserialize<'de>>(b: &'de [u8]) -> Result<T, Error> {
    from_slice(b)
}

pub fn from_reader<R: std::io::Read, T: DeserializeOwned>(mut r: R) -> Result<T, Error> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf).map_err(Error::Io)?;
    from_reader_inner(&buf)
}

macro_rules! deserialize_int {
    ($method:ident, $num_ty:ty, $visit:ident, $cast:ty) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            let bytes = self.scanner.read_number_bytes()?;
            visitor.$visit(parse_num::<$num_ty>(bytes)? as $cast)
        }
    };
}

impl<'de, 'a> de_trait::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let b = self.scanner.peek_byte_after_ws()?;
        match b {
            b'"'               => self.deserialize_str(visitor),
            b'{'               => self.deserialize_map(visitor),
            b'['               => self.deserialize_seq(visitor),
            b't' | b'f'        => self.deserialize_bool(visitor),
            b'n'               => { self.scanner.read_null()?; visitor.visit_unit() },
            b'-' | b'0'..=b'9' => self.deserialize_number(visitor),
            _                  => Err(Error::Scanner(jzon::Error::UnexpectedToken)),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let v = self.scanner.read_bool()?;
        visitor.visit_bool(v)
    }

    deserialize_int!(deserialize_i8,  i64, visit_i8,  i8);
    deserialize_int!(deserialize_i16, i64, visit_i16, i16);
    deserialize_int!(deserialize_i32, i64, visit_i32, i32);
    deserialize_int!(deserialize_i64, i64, visit_i64, i64);
    deserialize_int!(deserialize_u8,  u64, visit_u8,  u8);
    deserialize_int!(deserialize_u16, u64, visit_u16, u16);
    deserialize_int!(deserialize_u32, u64, visit_u32, u32);
    deserialize_int!(deserialize_u64, u64, visit_u64, u64);

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let bytes = self.scanner.read_number_bytes()?;
        let n: f64 = parse_f64(bytes)?;
        visitor.visit_f32(n as f32)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let bytes = self.scanner.read_number_bytes()?;
        let n: f64 = parse_f64(bytes)?;
        visitor.visit_f64(n)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.scanner.read_str()?;
        let st = s.as_str();
        let mut chars = st.chars();
        let c = chars.next().ok_or_else(|| Error::Custom("expected char".into()))?;
        visitor.visit_char(c)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.scanner.read_str()?;
        match s {
            JsonStr::Borrowed(b) | JsonStr::BorrowedNoEsc(b) => visitor.visit_borrowed_str(b),
            JsonStr::Owned(o)    => visitor.visit_string(o),
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.scanner.peek_null() {
            self.scanner.read_null()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.scanner.read_null()?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.scanner.skip_whitespace();
        self.scanner.expect_byte(b'[')?;
        let value = visitor.visit_seq(JsonSeqAccess { de: self, first: true, done: false })?;
        Ok(value)
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.scanner.skip_whitespace();
        self.scanner.expect_byte(b'{')?;
        let value = visitor.visit_map(JsonMapAccess { de: self, first: true, pending_value: false })?;
        Ok(value)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        let b = self.scanner.peek_byte_after_ws()?;
        if b == b'"' {
            let s = self.scanner.read_str()?;
            let variant = s.into_owned();
            visitor.visit_enum(StrDeserializer::new(variant))
        } else if b == b'{' {
            self.scanner.skip_whitespace();
            self.scanner.expect_byte(b'{')?;
            let value = visitor.visit_enum(JsonEnumAccess { de: self })?;
            self.scanner.skip_whitespace();
            self.scanner.expect_byte(b'}')?;
            Ok(value)
        } else {
            Err(Error::Scanner(jzon::Error::UnexpectedToken))
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.scanner.skip_value()?;
        visitor.visit_unit()
    }

    fn is_human_readable(&self) -> bool { true }
}

fn parse_num<T: std::str::FromStr>(bytes: &[u8]) -> Result<T, Error> {
    core::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or(Error::Scanner(jzon::Error::InvalidNumber))
}

fn parse_f64(bytes: &[u8]) -> Result<f64, Error> {
    let s = core::str::from_utf8(bytes).map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
    if let Ok(n) = s.parse::<i64>() {
        return Ok(n as f64);
    }
    s.parse::<f64>().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))
}

impl<'de, 'a> Deserializer<'de> {
    fn deserialize_number<V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value, Error> {
        let bytes = self.scanner.read_number_bytes()?;
        let is_float = bytes.iter().any(|&b| b == b'.' || b == b'e' || b == b'E');
        if is_float {
            let n = parse_f64(bytes)?;
            return visitor.visit_f64(n);
        }
        let s = core::str::from_utf8(bytes)
            .map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        if bytes[0] == b'-' {
            let n: i64 = s.parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
            visitor.visit_i64(n)
        } else {
            if let Ok(n) = s.parse::<u64>() {
                visitor.visit_u64(n)
            } else {
                let n: i64 = s.parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
                visitor.visit_i64(n)
            }
        }
    }
}

struct JsonSeqAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    first: bool,
    /// Set to true once we have consumed the closing `]`.
    done: bool,
}

impl<'de, 'a> SeqAccess<'de> for JsonSeqAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T: de_trait::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        self.de.scanner.skip_whitespace();
        match self.de.scanner.peek_byte() {
            Ok(b']') => { self.de.scanner.advance(); self.done = true; return Ok(None); }
            Err(_)   => return Err(Error::Scanner(jzon::Error::UnexpectedEof)),
            Ok(_)    => {}
        }
        if !self.first {
            self.de.scanner.expect_byte(b',')?;
            self.de.scanner.skip_whitespace();
            if self.de.scanner.peek_byte() == Ok(b']') {
                self.de.scanner.advance();
                self.done = true;
                return Ok(None);
            }
        }
        self.first = false;
        let value = seed.deserialize(&mut *self.de)?;
        Ok(Some(value))
    }
}

impl<'a, 'de> Drop for JsonSeqAccess<'a, 'de> {
    fn drop(&mut self) {
        if !self.done {
            // The visitor stopped early without draining all elements.
            // Consume remaining array elements and the closing `]` so the
            // scanner is positioned correctly for the caller.
            let _ = self.de.scanner.skip_array_tail();
        }
    }
}

struct JsonMapAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    first: bool,
    pending_value: bool,
}

impl<'de, 'a> MapAccess<'de> for JsonMapAccess<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K: de_trait::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        self.de.scanner.skip_whitespace();
        match self.de.scanner.peek_byte() {
            Ok(b'}') => { self.de.scanner.advance(); return Ok(None); }
            Err(_)   => return Err(Error::Scanner(jzon::Error::UnexpectedEof)),
            Ok(_)    => {}
        }
        if !self.first {
            self.de.scanner.expect_byte(b',')?;
            self.de.scanner.skip_whitespace();
            if self.de.scanner.peek_byte() == Ok(b'}') {
                self.de.scanner.advance();
                return Ok(None);
            }
        }
        self.first = false;
        self.pending_value = true;
        let key = seed.deserialize(MapKeyDeserializer { de: &mut *self.de })?;
        self.de.scanner.skip_whitespace();
        self.de.scanner.expect_byte(b':')?;
        Ok(Some(key))
    }

    fn next_value_seed<V: de_trait::DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Error> {
        self.pending_value = false;
        seed.deserialize(&mut *self.de)
    }
}

// Keys in JSON maps are always strings.
struct MapKeyDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> de_trait::Deserializer<'de> for MapKeyDeserializer<'a, 'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        match s {
            JsonStr::Borrowed(b) | JsonStr::BorrowedNoEsc(b) => visitor.visit_borrowed_str(b),
            JsonStr::Owned(o)    => visitor.visit_string(o),
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: i8 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_i8(n)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: i16 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_i16(n)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: i32 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_i32(n)
    }
    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: i64 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_i64(n)
    }
    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: u8 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_u8(n)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: u16 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_u16(n)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: u32 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_u32(n)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: u64 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_u64(n)
    }
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let b: bool = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::UnexpectedToken))?;
        visitor.visit_bool(b)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> { self.deserialize_str(visitor) }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> { self.deserialize_str(visitor) }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> { visitor.visit_some(self) }
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> { visitor.visit_unit() }
    fn deserialize_unit_struct<V: Visitor<'de>>(self, _: &'static str, visitor: V) -> Result<V::Value, Error> { visitor.visit_unit() }
    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _: &'static str, visitor: V) -> Result<V::Value, Error> { visitor.visit_newtype_struct(self) }
    fn deserialize_seq<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> { Err(Error::Custom("seq key not supported".into())) }
    fn deserialize_tuple<V: Visitor<'de>>(self, _: usize, _visitor: V) -> Result<V::Value, Error> { Err(Error::Custom("tuple key not supported".into())) }
    fn deserialize_tuple_struct<V: Visitor<'de>>(self, _: &'static str, _: usize, _visitor: V) -> Result<V::Value, Error> { Err(Error::Custom("tuple_struct key not supported".into())) }
    fn deserialize_map<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> { Err(Error::Custom("map key not supported".into())) }
    fn deserialize_struct<V: Visitor<'de>>(self, _: &'static str, _: &'static [&'static str], _visitor: V) -> Result<V::Value, Error> { Err(Error::Custom("struct key not supported".into())) }
    fn deserialize_enum<V: Visitor<'de>>(self, _: &'static str, _: &'static [&'static str], visitor: V) -> Result<V::Value, Error> { self.deserialize_str(visitor) }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> { self.deserialize_str(visitor) }
    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: f32 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_f32(n)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let s = self.de.scanner.read_str()?;
        let n: f64 = s.as_str().parse().map_err(|_| Error::Scanner(jzon::Error::InvalidNumber))?;
        visitor.visit_f64(n)
    }
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> { self.deserialize_str(visitor) }
}

struct StrDeserializer {
    value: String,
}

impl StrDeserializer {
    fn new(value: String) -> Self { StrDeserializer { value } }
}

impl<'de> de_trait::Deserializer<'de> for StrDeserializer {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(self.value)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(self.value)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(self.value)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(self.value)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_enum(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char bytes byte_buf option
        unit unit_struct newtype_struct seq tuple tuple_struct map struct
        ignored_any
    }
}

impl<'de> EnumAccess<'de> for StrDeserializer {
    type Error = Error;
    type Variant = UnitOnlyVariantAccess;

    fn variant_seed<V: de_trait::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let val = seed.deserialize(StrDeserializer::new(self.value))?;
        Ok((val, UnitOnlyVariantAccess))
    }
}

struct UnitOnlyVariantAccess;

impl<'de> VariantAccess<'de> for UnitOnlyVariantAccess {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> { Ok(()) }

    fn newtype_variant_seed<T: de_trait::DeserializeSeed<'de>>(
        self,
        _seed: T,
    ) -> Result<T::Value, Error> {
        Err(Error::Custom("expected unit variant".into()))
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Error> {
        Err(Error::Custom("expected unit variant".into()))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Error> {
        Err(Error::Custom("expected unit variant".into()))
    }
}

struct JsonEnumAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> EnumAccess<'de> for JsonEnumAccess<'a, 'de> {
    type Error = Error;
    type Variant = JsonVariantAccess<'a, 'de>;

    fn variant_seed<V: de_trait::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let s = self.de.scanner.read_str()?;
        let variant_name = s.into_owned();
        self.de.scanner.skip_whitespace();
        self.de.scanner.expect_byte(b':')?;
        let val = seed.deserialize(StrDeserializer::new(variant_name))?;
        Ok((val, JsonVariantAccess { de: self.de }))
    }
}

struct JsonVariantAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> VariantAccess<'de> for JsonVariantAccess<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        self.de.scanner.read_null()?;
        Ok(())
    }

    fn newtype_variant_seed<T: de_trait::DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Error> {
        seed.deserialize(&mut *self.de)
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        de_trait::Deserializer::deserialize_seq(&mut *self.de, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        de_trait::Deserializer::deserialize_map(&mut *self.de, visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Point { x: f64, y: f64 }

    #[test]
    fn point_roundtrip() {
        let p = Point { x: 1.5, y: -2.0 };
        let json = to_string(&p).unwrap();
        let p2: Point = from_str(&json).unwrap();
        assert_eq!(p, p2);
    }

    #[test]
    fn string_zero_copy() {
        let json = r#"{"x":1.0,"y":2.5}"#;
        let p: Point = from_str(json).unwrap();
        assert!((p.x - 1.0).abs() < 1e-12);
        assert!((p.y - 2.5).abs() < 1e-12);
    }

    #[test]
    fn nested_map() {
        use std::collections::HashMap;
        let mut m = HashMap::new();
        m.insert("a".to_string(), vec![1u64, 2, 3]);
        let json = to_string(&m).unwrap();
        let m2: HashMap<String, Vec<u64>> = from_str(&json).unwrap();
        assert_eq!(m["a"], m2["a"]);
    }

    #[test]
    fn serde_json_compat() {
        let v = vec![1u64, 2, 3];
        assert_eq!(to_string(&v).unwrap(), serde_json::to_string(&v).unwrap());
    }

    #[test]
    fn bool_roundtrip() {
        assert_eq!(to_string(&true).unwrap(), "true");
        assert_eq!(to_string(&false).unwrap(), "false");
        let b: bool = from_str("true").unwrap();
        assert!(b);
    }

    #[test]
    fn option_roundtrip() {
        let v: Option<u64> = Some(42);
        let json = to_string(&v).unwrap();
        let v2: Option<u64> = from_str(&json).unwrap();
        assert_eq!(v, v2);

        let v: Option<u64> = None;
        let json = to_string(&v).unwrap();
        assert_eq!(json, "null");
        let v2: Option<u64> = from_str(&json).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn string_escape_roundtrip() {
        let s = "hello \"world\"\nnewline\ttab\\backslash";
        let json = to_string(&s.to_string()).unwrap();
        let s2: String = from_str(&json).unwrap();
        assert_eq!(s, s2);
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum Color { Red, Green, Blue }

    #[test]
    fn unit_enum_roundtrip() {
        let c = Color::Green;
        let json = to_string(&c).unwrap();
        assert_eq!(json, r#""Green""#);
        let c2: Color = from_str(&json).unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn integer_types() {
        assert_eq!(to_string(&42u8).unwrap(), "42");
        assert_eq!(to_string(&-1i32).unwrap(), "-1");
        assert_eq!(to_string(&u64::MAX).unwrap(), u64::MAX.to_string());
        let n: u64 = from_str("18446744073709551615").unwrap();
        assert_eq!(n, u64::MAX);
    }
}
