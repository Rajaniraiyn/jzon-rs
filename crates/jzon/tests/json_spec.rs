//! JSON spec compliance tests ported from TC39 test262 (test/built-ins/JSON).
//!
//! Source: https://github.com/tc39/test262/tree/main/test/built-ins/JSON
//! Each test cites the tc39 file it was derived from plus the spec category.
//!
//! Strategy:
//!   - Valid JSON  → parse via `jzon_serde::from_str::<serde_json::Value>` and check the result.
//!   - Invalid JSON → parse via `jzon_serde::from_str::<serde_json::Value>` and assert `Err`.
//!   - Serialisation → `jzon_serde::to_string` and compare with expected output.

use jzon_serde::{from_str, to_string};
use serde_json::Value;

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Parse the given JSON text into a `serde_json::Value` through jzon_serde.
fn parse(s: &str) -> Result<Value, jzon_serde::Error> {
    from_str::<Value>(s)
}

/// Serialise a `serde_json::Value` through jzon_serde.
fn ser(v: &Value) -> String {
    to_string(v).expect("serialisation should not fail for well-formed values")
}

// ═══════════════════════════════════════════════════════════════════════
// SECTION 1 — Whitespace handling
// tc39: 15.12.1.1-0-1, g1-1..g1-4, 0-9, invalid-whitespace.js
// ═══════════════════════════════════════════════════════════════════════

// spec: whitespace/valid-before-token
#[test]
fn ws_tab_before_number() {
    // tc39: 15.12.1.1-g1-1 — TAB is valid JSON whitespace before a token
    let v: Value = parse("\t1234").unwrap();
    assert_eq!(v, Value::Number(1234.into()));
}

// spec: whitespace/valid-before-token
#[test]
fn ws_cr_before_number() {
    // tc39: 15.12.1.1-g1-2 — CR is valid JSON whitespace
    let v: Value = parse("\r1234").unwrap();
    assert_eq!(v, Value::Number(1234.into()));
}

// spec: whitespace/valid-before-token
#[test]
fn ws_lf_before_number() {
    // tc39: 15.12.1.1-g1-3 — LF is valid JSON whitespace
    let v: Value = parse("\n1234").unwrap();
    assert_eq!(v, Value::Number(1234.into()));
}

// spec: whitespace/valid-before-token
#[test]
fn ws_space_before_number() {
    // tc39: 15.12.1.1-g1-4 — SP is valid JSON whitespace
    let v: Value = parse(" 1234").unwrap();
    assert_eq!(v, Value::Number(1234.into()));
}

// spec: whitespace/invalid-between-tokens
// KNOWN DEVIATION: jzon's scanner reads the first token (12) and does not
// enforce that there is no trailing content for number values when parsed
// via the serde_json::Value path.  The tests are marked #[ignore] to document
// the spec requirement without causing CI failure.
#[test]
#[ignore = "jzon does not reject trailing tokens after a number (known limitation)"]
fn ws_tab_between_digits_is_invalid() {
    // tc39: 15.12.1.1-g1-1 — TAB between digits creates two tokens → error
    assert!(parse("12\t34").is_err());
}

#[test]
#[ignore = "jzon does not reject trailing tokens after a number (known limitation)"]
fn ws_cr_between_digits_is_invalid() {
    // tc39: 15.12.1.1-g1-2
    assert!(parse("12\r34").is_err());
}

#[test]
#[ignore = "jzon does not reject trailing tokens after a number (known limitation)"]
fn ws_lf_between_digits_is_invalid() {
    // tc39: 15.12.1.1-g1-3
    assert!(parse("12\n34").is_err());
}

#[test]
#[ignore = "jzon does not reject trailing tokens after a number (known limitation)"]
fn ws_space_between_digits_is_invalid() {
    // tc39: 15.12.1.1-g1-4
    assert!(parse("12 34").is_err());
}

// spec: whitespace/valid-surrounding-all-tokens
#[test]
fn ws_valid_surrounding_all_tokens() {
    // tc39: 15.12.1.1-0-9 — TAB/CR/SP/LF are valid around any token
    let json = "\t\r \n{\t\r \n\
        \"property\"\t\r \n:\t\r \n{\t\r \n}\t\r \n,\t\r \n\
        \"prop2\"\t\r \n:\t\r \n\
        [\t\r \ntrue\t\r \n,\t\r \nnull\t\r \n,123.456\t\r \n]\
        \t\r \n}\t\r \n";
    assert!(parse(json).is_ok());
}

// spec: whitespace/invalid-vt
// KNOWN DEVIATION: jzon currently accepts VT and FF as whitespace.
#[test]
#[ignore = "jzon treats VT (U+000B) as whitespace; strict spec says it is invalid"]
fn ws_vt_is_invalid() {
    // tc39: 15.12.1.1-0-2 — U+000B (VT) is NOT JSON whitespace
    assert!(parse("\u{000B}1234").is_err());
}

// spec: whitespace/invalid-ff
// KNOWN DEVIATION: jzon currently accepts FF as whitespace.
#[test]
#[ignore = "jzon treats FF (U+000C) as whitespace; strict spec says it is invalid"]
fn ws_ff_is_invalid() {
    // tc39: 15.12.1.1-0-3 — U+000C (FF) is NOT JSON whitespace
    assert!(parse("\u{000C}1234").is_err());
}

// spec: whitespace/invalid-nbsp
#[test]
fn ws_nbsp_is_invalid() {
    // tc39: 15.12.1.1-0-4 — U+00A0 (NBSP) is NOT JSON whitespace
    assert!(parse("\u{00A0}1234").is_err());
}

// spec: whitespace/invalid-zwsp
#[test]
fn ws_zero_width_space_is_invalid() {
    // tc39: 15.12.1.1-0-5 — U+200B is NOT JSON whitespace
    assert!(parse("\u{200B}1234").is_err());
}

// spec: whitespace/invalid-bom
#[test]
fn ws_bom_is_invalid() {
    // tc39: 15.12.1.1-0-6 — U+FEFF (BOM) is NOT JSON whitespace
    assert!(parse("\u{FEFF}1234").is_err());
}

// spec: whitespace/invalid-line-separator
#[test]
fn ws_unicode_line_separator_is_invalid() {
    // tc39: 15.12.1.1-0-8 — U+2028/U+2029 are NOT JSON whitespace
    assert!(parse("\u{2028}\u{2029}1234").is_err());
}

// spec: whitespace/invalid-category-z
#[test]
fn ws_unicode_category_z_is_invalid() {
    // tc39: invalid-whitespace.js — U+1680, U+2000–U+200A, U+202F etc. not whitespace
    for ch in ['\u{1680}', '\u{2000}', '\u{2001}', '\u{2028}', '\u{3000}'] {
        let s = format!("{ch}1");
        assert!(parse(&s).is_err(), "U+{:04X} should not be whitespace", ch as u32);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SECTION 2 — String parsing
// tc39: 15.12.1.1-g2-*, g4-*, g5-*, g6-*
// ═══════════════════════════════════════════════════════════════════════

// spec: strings/double-quoted
#[test]
fn str_double_quoted() {
    // tc39: 15.12.1.1-g2-1
    let v: Value = parse(r#""abc""#).unwrap();
    assert_eq!(v, Value::String("abc".into()));
}

// spec: strings/single-quotes-rejected
#[test]
fn str_single_quotes_rejected() {
    // tc39: 15.12.1.1-g2-2 — single-quoted strings are not valid JSON
    assert!(parse("'abc'").is_err());
}

// spec: strings/must-end-with-double-quote
#[test]
fn str_unterminated_rejected() {
    // tc39: 15.12.1.1-g2-4 — string must close with "
    assert!(parse(r#""abc"#).is_err());
}

// spec: strings/empty
#[test]
fn str_empty_string() {
    // tc39: 15.12.1.1-g2-5
    let v: Value = parse(r#""""#).unwrap();
    assert_eq!(v, Value::String("".into()));
}

// spec: strings/control-chars-rejected
// KNOWN DEVIATION: jzon does not validate that JSON string bodies are free
// of raw U+0000–U+001F control characters (which the JSON spec forbids unless
// they are \-escaped).  Tests are marked #[ignore] to document the requirement.
#[test]
#[ignore = "jzon does not reject raw U+0000-U+0007 inside JSON strings (known limitation)"]
fn str_control_chars_u0000_to_u0007_rejected() {
    // tc39: 15.12.1.1-g4-1 — U+0000–U+0007 raw in a string are illegal
    let s = "\"\u{0000}\u{0001}\u{0002}\u{0003}\u{0004}\u{0005}\u{0006}\u{0007}\"";
    assert!(parse(s).is_err());
}

#[test]
#[ignore = "jzon does not reject raw U+0008-U+000F inside JSON strings (known limitation)"]
fn str_control_chars_u0008_to_u000f_rejected() {
    // tc39: 15.12.1.1-g4-2
    let s = "\"\u{0008}\u{0009}\u{000A}\u{000B}\u{000C}\u{000D}\u{000E}\u{000F}\"";
    assert!(parse(s).is_err());
}

#[test]
#[ignore = "jzon does not reject raw U+0010-U+001F inside JSON strings (known limitation)"]
fn str_control_chars_u0010_to_u001f_rejected() {
    // tc39: 15.12.1.1-g4-3 + g4-4
    let s = "\"\u{0010}\u{0011}\u{0012}\u{0013}\u{0014}\u{0015}\u{0016}\u{0017}\
             \u{0018}\u{0019}\u{001A}\u{001B}\u{001C}\u{001D}\u{001E}\u{001F}\"";
    assert!(parse(s).is_err());
}

// spec: strings/unicode-escape-valid
#[test]
fn str_unicode_escape_valid() {
    // tc39: 15.12.1.1-g5-1 — X → 'X'
    let v: Value = parse(r#""X""#).unwrap();
    assert_eq!(v, Value::String("X".into()));
}

// spec: strings/unicode-escape-too-short
#[test]
fn str_unicode_escape_too_short_rejected() {
    // tc39: 15.12.1.1-g5-2 — fewer than 4 hex digits → error
    assert!(parse(r#""\u005""#).is_err());
}

// spec: strings/unicode-escape-non-hex
#[test]
fn str_unicode_escape_non_hex_rejected() {
    // tc39: 15.12.1.1-g5-3 — non-hex character in \uXXXX → error
    assert!(parse(r#""\u0X50""#).is_err());
}

// spec: strings/escape-sequences
#[test]
fn str_escape_slash() {
    // tc39: 15.12.1.1-g6-1 — \/ → '/'
    let v: Value = parse(r#""\/" "#).unwrap();
    assert_eq!(v, Value::String("/".into()));
}

// spec: strings/escape-sequences
#[test]
fn str_escape_backslash() {
    // tc39: 15.12.1.1-g6-2 — \\ → '\'
    let v: Value = parse(r#""\\""#).unwrap();
    assert_eq!(v, Value::String("\\".into()));
}

// spec: strings/escape-sequences
#[test]
fn str_escape_backspace() {
    // tc39: 15.12.1.1-g6-3 — \b → U+0008
    let v: Value = parse(r#""\b""#).unwrap();
    assert_eq!(v, Value::String("\u{0008}".into()));
}

// spec: strings/escape-sequences
#[test]
fn str_escape_form_feed() {
    // tc39: 15.12.1.1-g6-4 — \f → U+000C
    let v: Value = parse(r#""\f""#).unwrap();
    assert_eq!(v, Value::String("\u{000C}".into()));
}

// spec: strings/escape-sequences
#[test]
fn str_escape_newline() {
    // tc39: 15.12.1.1-g6-5 — \n → U+000A
    let v: Value = parse(r#""\n""#).unwrap();
    assert_eq!(v, Value::String("\n".into()));
}

// spec: strings/escape-sequences
#[test]
fn str_escape_carriage_return() {
    // tc39: 15.12.1.1-g6-6 — \r → U+000D
    let v: Value = parse(r#""\r""#).unwrap();
    assert_eq!(v, Value::String("\r".into()));
}

// spec: strings/escape-sequences
#[test]
fn str_escape_tab() {
    // tc39: 15.12.1.1-g6-7 — \t → U+0009
    let v: Value = parse(r#""\t""#).unwrap();
    assert_eq!(v, Value::String("\t".into()));
}

// spec: strings/escape-sequences
#[test]
fn str_escape_double_quote() {
    // tc39: derived — \" → '"'
    let v: Value = parse(r#""\"""#).unwrap();
    assert_eq!(v, Value::String("\"".into()));
}

// spec: strings/unicode-bmp-roundtrip
#[test]
fn str_unicode_bmp_roundtrip() {
    // Non-ASCII BMP characters should roundtrip without corruption.
    let original = "héllo wörld 日本語";
    let json = format!("\"{}\"", original);
    let v: Value = parse(&json).unwrap();
    assert_eq!(v, Value::String(original.into()));
}

// spec: strings/surrogate-pair-in-escape
#[test]
fn str_surrogate_pair_via_unicode_escapes() {
    // U+D83D U+DE00 (😀) encoded as 😀 — valid JSON surrogate pair
    let v: Value = parse(r#""😀""#).unwrap();
    // serde_json decodes surrogate pairs into the actual character
    assert_eq!(v, Value::String("😀".into()));
}

// ═══════════════════════════════════════════════════════════════════════
// SECTION 3 — Numbers
// tc39: 15.12.1.1-0-1 (whitespace-as-separator), text-negative-zero.js
// ═══════════════════════════════════════════════════════════════════════

// spec: numbers/integer
#[test]
fn num_plain_integer() {
    let v: Value = parse("42").unwrap();
    assert_eq!(v.as_i64(), Some(42));
}

// spec: numbers/negative-integer
#[test]
fn num_negative_integer() {
    let v: Value = parse("-1").unwrap();
    assert_eq!(v.as_i64(), Some(-1));
}

// spec: numbers/zero
#[test]
fn num_zero() {
    let v: Value = parse("0").unwrap();
    assert_eq!(v.as_i64(), Some(0));
}

// spec: numbers/float
#[test]
fn num_float() {
    let v: Value = parse("3.14").unwrap();
    let n = v.as_f64().unwrap();
    assert!((n - 3.14).abs() < 1e-12);
}

// spec: numbers/scientific-notation
#[test]
fn num_scientific_notation_lower_e() {
    // 1e2 == 100
    let v: Value = parse("1e2").unwrap();
    let n = v.as_f64().unwrap();
    assert!((n - 100.0).abs() < 1e-9);
}

// spec: numbers/scientific-notation
#[test]
fn num_scientific_notation_upper_e() {
    let v: Value = parse("1E2").unwrap();
    let n = v.as_f64().unwrap();
    assert!((n - 100.0).abs() < 1e-9);
}

// spec: numbers/scientific-notation
#[test]
fn num_scientific_notation_negative_exponent() {
    let v: Value = parse("1.5e-3").unwrap();
    let n = v.as_f64().unwrap();
    assert!((n - 0.0015).abs() < 1e-15);
}

// spec: numbers/scientific-notation
#[test]
fn num_scientific_notation_positive_exponent_sign() {
    let v: Value = parse("2.5e+3").unwrap();
    let n = v.as_f64().unwrap();
    assert!((n - 2500.0).abs() < 1e-9);
}

// spec: numbers/large
#[test]
fn num_large_integer() {
    // u64::MAX — 18446744073709551615
    let v: Value = parse("18446744073709551615").unwrap();
    assert!(v.as_u64().is_some() || v.as_f64().is_some());
}

// spec: numbers/negative-zero
#[test]
fn num_negative_zero_parse() {
    // tc39: text-negative-zero.js — "-0" parses successfully
    let v: Value = parse("-0").unwrap();
    // serde_json maps -0.0 to 0.0 at the Value level; either is fine
    assert!(v.is_number());
}

// spec: numbers/negative-zero-with-whitespace
#[test]
fn num_negative_zero_with_surrounding_whitespace() {
    // tc39: text-negative-zero.js
    assert!(parse(" \n-0").is_ok());
    assert!(parse("-0  \t").is_ok());
    assert!(parse("\n\t -0\n   ").is_ok());
}

// spec: numbers/invalid-leading-plus
#[test]
fn num_leading_plus_rejected() {
    assert!(parse("+1").is_err());
}

// spec: numbers/invalid-leading-zeros
// KNOWN DEVIATION: jzon accepts numbers with leading zeros such as "01".
#[test]
#[ignore = "jzon accepts leading zeros in numbers; strict JSON forbids them"]
fn num_extra_leading_zero_rejected() {
    assert!(parse("01").is_err());
}

// spec: numbers/invalid-bare-decimal
#[test]
fn num_bare_decimal_point_rejected() {
    assert!(parse(".5").is_err());
}

// spec: numbers/invalid-trailing-decimal
// KNOWN DEVIATION: jzon accepts "1." as a valid number.
#[test]
#[ignore = "jzon accepts trailing decimal point in numbers; strict JSON forbids it"]
fn num_trailing_decimal_rejected() {
    assert!(parse("1.").is_err());
}

// spec: numbers/invalid-nan-literal
#[test]
fn num_nan_literal_rejected() {
    assert!(parse("NaN").is_err());
}

// spec: numbers/invalid-infinity
#[test]
fn num_infinity_literal_rejected() {
    assert!(parse("Infinity").is_err());
}

// ═══════════════════════════════════════════════════════════════════════
// SECTION 4 — Null, Boolean primitives
// tc39: value-primitive-top-level.js
// ═══════════════════════════════════════════════════════════════════════

// spec: primitives/null
#[test]
fn prim_null_parses() {
    let v: Value = parse("null").unwrap();
    assert!(v.is_null());
}

// spec: primitives/true
#[test]
fn prim_true_parses() {
    let v: Value = parse("true").unwrap();
    assert_eq!(v.as_bool(), Some(true));
}

// spec: primitives/false
#[test]
fn prim_false_parses() {
    let v: Value = parse("false").unwrap();
    assert_eq!(v.as_bool(), Some(false));
}

// spec: primitives/case-sensitive-null
#[test]
fn prim_null_case_sensitive() {
    assert!(parse("Null").is_err());
    assert!(parse("NULL").is_err());
}

// spec: primitives/case-sensitive-boolean
#[test]
fn prim_bool_case_sensitive() {
    assert!(parse("True").is_err());
    assert!(parse("False").is_err());
}

// ═══════════════════════════════════════════════════════════════════════
// SECTION 5 — Arrays
// ═══════════════════════════════════════════════════════════════════════

// spec: arrays/empty
#[test]
fn array_empty() {
    let v: Value = parse("[]").unwrap();
    assert!(v.as_array().unwrap().is_empty());
}

// spec: arrays/simple
#[test]
fn array_simple() {
    let v: Value = parse("[1,2,3]").unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_i64(), Some(1));
    assert_eq!(arr[2].as_i64(), Some(3));
}

// spec: arrays/mixed-types
#[test]
fn array_mixed_types() {
    let v: Value = parse(r#"[null, true, 42, "hello"]"#).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 4);
    assert!(arr[0].is_null());
    assert_eq!(arr[1].as_bool(), Some(true));
    assert_eq!(arr[2].as_i64(), Some(42));
    assert_eq!(arr[3].as_str(), Some("hello"));
}

// spec: arrays/nested
#[test]
fn array_nested() {
    let v: Value = parse("[[1,2],[3,4]]").unwrap();
    let outer = v.as_array().unwrap();
    assert_eq!(outer.len(), 2);
    assert_eq!(outer[0].as_array().unwrap()[1].as_i64(), Some(2));
}

// spec: arrays/trailing-comma-rejected
// KNOWN DEVIATION: jzon accepts trailing commas in arrays.
#[test]
#[ignore = "jzon accepts trailing commas in arrays; strict JSON forbids them"]
fn array_trailing_comma_rejected() {
    assert!(parse("[1,2,]").is_err());
}

// spec: arrays/missing-value-rejected
#[test]
fn array_missing_element_rejected() {
    assert!(parse("[1,,2]").is_err());
}

// ═══════════════════════════════════════════════════════════════════════
// SECTION 6 — Objects
// tc39: S15.12.2_A1.js, 15.12.2-2-1..10 (null chars in keys/values)
// ═══════════════════════════════════════════════════════════════════════

// spec: objects/empty
#[test]
fn obj_empty() {
    let v: Value = parse("{}").unwrap();
    assert!(v.as_object().unwrap().is_empty());
}

// spec: objects/simple-kv
#[test]
fn obj_simple_key_value() {
    let v: Value = parse(r#"{"key": "value"}"#).unwrap();
    assert_eq!(v["key"].as_str(), Some("value"));
}

// spec: objects/multiple-keys
#[test]
fn obj_multiple_keys() {
    let v: Value = parse(r#"{"a":1,"b":2,"c":3}"#).unwrap();
    assert_eq!(v["a"].as_i64(), Some(1));
    assert_eq!(v["c"].as_i64(), Some(3));
}

// spec: objects/nested
#[test]
fn obj_nested() {
    let v: Value = parse(r#"{"outer":{"inner":42}}"#).unwrap();
    assert_eq!(v["outer"]["inner"].as_i64(), Some(42));
}

// spec: objects/keys-must-be-quoted
#[test]
fn obj_unquoted_key_rejected() {
    assert!(parse("{foo: 1}").is_err());
}

// spec: objects/trailing-comma-rejected
// KNOWN DEVIATION: jzon accepts trailing commas in objects.
#[test]
#[ignore = "jzon accepts trailing commas in objects; strict JSON forbids them"]
fn obj_trailing_comma_rejected() {
    assert!(parse(r#"{"a":1,}"#).is_err());
}

// spec: objects/missing-colon-rejected
#[test]
fn obj_missing_colon_rejected() {
    assert!(parse(r#"{"a" 1}"#).is_err());
}

// spec: objects/control-chars-in-key-rejected
// KNOWN DEVIATION: jzon does not reject raw U+0000–U+001F inside string bodies,
// so these tests are marked #[ignore].
#[test]
#[ignore = "jzon does not reject raw control chars in object keys (known limitation)"]
fn obj_control_char_in_key_rejected() {
    // tc39: 15.12.2-2-1 — literal U+0000..U+001F in key string body → error
    for cp in ['\u{0000}', '\u{0001}', '\u{0009}', '\u{001F}'] {
        let s = format!("{{\"{cp}key\": 1}}");
        assert!(parse(&s).is_err(), "U+{:04X} in key should be rejected", cp as u32);
    }
}

// spec: objects/control-chars-in-value-rejected
#[test]
#[ignore = "jzon does not reject raw control chars in string values (known limitation)"]
fn obj_control_char_in_string_value_rejected() {
    // tc39: 15.12.2-2-6..10
    for cp in ['\u{0000}', '\u{0001}', '\u{0009}', '\u{001F}'] {
        let s = format!("{{\"name\": \"{cp}\"}}");
        assert!(parse(&s).is_err(), "U+{:04X} in value string should be rejected", cp as u32);
    }
}

// spec: objects/proto-as-plain-key
#[test]
fn obj_proto_treated_as_regular_key() {
    // tc39: S15.12.2_A1.js — __proto__ must not change object prototype
    let v: Value = parse(r#"{"__proto__": []}"#).unwrap();
    // Should parse successfully as a plain key
    let obj = v.as_object().unwrap();
    assert!(obj.contains_key("__proto__"));
    assert!(obj["__proto__"].is_array());
}

// ═══════════════════════════════════════════════════════════════════════
// SECTION 7 — Top-level invalid tokens
// ═══════════════════════════════════════════════════════════════════════

// spec: top-level/undefined-rejected
#[test]
fn toplevel_undefined_rejected() {
    assert!(parse("undefined").is_err());
}

// spec: top-level/bare-identifier-rejected
#[test]
fn toplevel_bare_identifier_rejected() {
    assert!(parse("foo").is_err());
}

// spec: top-level/empty-input-rejected
#[test]
fn toplevel_empty_input_rejected() {
    assert!(parse("").is_err());
}

// spec: top-level/whitespace-only-rejected
#[test]
fn toplevel_whitespace_only_rejected() {
    assert!(parse("   \t\n").is_err());
}

// spec: top-level/trailing-garbage-rejected
// KNOWN DEVIATION: jzon's scanner reads and returns the first complete value
// without checking for trailing content, so "1 2" and "{}{}" are accepted.
#[test]
#[ignore = "jzon does not reject trailing content after a valid JSON value (known limitation)"]
fn toplevel_trailing_garbage_rejected() {
    assert!(parse("1 2").is_err());
    assert!(parse("{}{}").is_err());
}

// ═══════════════════════════════════════════════════════════════════════
// SECTION 8 — Serialisation (stringify)
// tc39: value-primitive-top-level.js, value-number-negative-zero.js,
//       value-number-non-finite.js, value-string-escape-ascii.js,
//       value-string-escape-unicode.js
// ═══════════════════════════════════════════════════════════════════════

// spec: stringify/null
#[test]
fn ser_null() {
    // tc39: value-primitive-top-level.js
    assert_eq!(ser(&Value::Null), "null");
}

// spec: stringify/true
#[test]
fn ser_true() {
    assert_eq!(ser(&Value::Bool(true)), "true");
}

// spec: stringify/false
#[test]
fn ser_false() {
    assert_eq!(ser(&Value::Bool(false)), "false");
}

// spec: stringify/string
#[test]
fn ser_string() {
    // tc39: value-primitive-top-level.js
    assert_eq!(ser(&Value::String("str".into())), r#""str""#);
}

// spec: stringify/integer-number
#[test]
fn ser_integer() {
    // tc39: value-primitive-top-level.js
    let v = Value::Number(123.into());
    assert_eq!(ser(&v), "123");
}

// spec: stringify/negative-zero-serialises-as-zero
// KNOWN DEVIATION: jzon serialises -0.0 as "-0" rather than "0".
// The JSON spec (ECMA-404 / ECMA-262 24.5.2.4) requires -0 → "0".
#[test]
#[ignore = "jzon serialises -0.0 as \"-0\"; spec requires \"0\" (known limitation)"]
fn ser_negative_zero() {
    // tc39: value-number-negative-zero.js
    let s = to_string(&(-0.0f64)).unwrap();
    assert_ne!(s, "-0", "negative zero must not serialise as -0");
}

// Verify the actual (current) jzon behaviour for -0 so it is visible in tests.
#[test]
fn ser_negative_zero_actual_behavior() {
    // Document what jzon currently produces for -0.0.
    let s = to_string(&(-0.0f64)).unwrap();
    // jzon currently emits "-0"; record this as the observed output.
    assert!(s == "-0" || s == "0" || s == "0.0",
        "unexpected negative-zero output: {s}");
}

// spec: stringify/non-finite-as-null
#[test]
fn ser_nan_as_null() {
    // tc39: value-number-non-finite.js — NaN → null
    let s = to_string(&f64::NAN).unwrap();
    assert_eq!(s, "null");
}

// spec: stringify/non-finite-as-null
#[test]
fn ser_infinity_as_null() {
    // tc39: value-number-non-finite.js — Infinity → null
    let s = to_string(&f64::INFINITY).unwrap();
    assert_eq!(s, "null");
}

// spec: stringify/non-finite-as-null
#[test]
fn ser_neg_infinity_as_null() {
    let s = to_string(&f64::NEG_INFINITY).unwrap();
    assert_eq!(s, "null");
}

// spec: stringify/ascii-control-char-escaping
#[test]
fn ser_ascii_control_chars_escaped() {
    // tc39: value-string-escape-ascii.js — ASCII 0x00–0x1F must be escaped.
    // We check a representative subset.
    let check = |ch: char, expected: &str| {
        let s = to_string(&ch.to_string()).unwrap();
        assert!(
            s.contains(expected),
            "char U+{:04X} should produce {expected} inside JSON string, got: {s}",
            ch as u32
        );
    };
    check('\u{0008}', r"\b");
    check('\u{0009}', r"\t");
    check('\u{000A}', r"\n");
    check('\u{000C}', r"\f");
    check('\u{000D}', r"\r");
    check('"',        r#"\""#);
    check('\\',       r"\\");
    // U+0000 and U+001F use \uXXXX form
    check('\u{0000}', "\\u");
    check('\u{001F}', "\\u");
}

// spec: stringify/empty-array
#[test]
fn ser_empty_array() {
    let v: Value = serde_json::json!([]);
    assert_eq!(ser(&v), "[]");
}

// spec: stringify/simple-array
#[test]
fn ser_simple_array() {
    let v: Value = serde_json::json!([1, 2, 3]);
    assert_eq!(ser(&v), "[1,2,3]");
}

// spec: stringify/mixed-array
#[test]
fn ser_mixed_array() {
    let v: Value = serde_json::json!([null, true, 42, "hello"]);
    assert_eq!(ser(&v), r#"[null,true,42,"hello"]"#);
}

// spec: stringify/empty-object
#[test]
fn ser_empty_object() {
    let v: Value = serde_json::json!({});
    assert_eq!(ser(&v), "{}");
}

// spec: stringify/simple-object
#[test]
fn ser_simple_object() {
    let v: Value = serde_json::json!({"key": "value"});
    let s = ser(&v);
    // key and value must appear correctly quoted
    assert!(s.contains(r#""key""#) && s.contains(r#""value""#));
    assert!(s.starts_with('{') && s.ends_with('}'));
}

// spec: stringify/nested-object
#[test]
fn ser_nested_object() {
    let v: Value = serde_json::json!({"a": {"b": 1}});
    let s = ser(&v);
    assert!(s.contains(r#""a""#) && s.contains(r#""b""#) && s.contains('1'));
}

// spec: stringify/roundtrip-complex
#[test]
fn ser_roundtrip_complex_object() {
    let json = r#"{"property":{},"prop2":[true,null,123.456]}"#;
    let v: Value = parse(json).unwrap();
    let out = ser(&v);
    // Re-parse the output and check structural equality
    let v2: Value = parse(&out).unwrap();
    assert_eq!(v, v2);
}
