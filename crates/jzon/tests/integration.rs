//! Integration tests covering all supported struct syntaxes, serde attributes,
//! data types, and runtime properties (zero-copy, allocation behaviour).

use jzon::{FromJson, ToJson};
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}
#[test]
fn basic_roundtrip() {
    let p = Point { x: 1.5, y: -2.0 };
    let json = p.to_json_string();
    let p2 = Point::from_json_str(&json).unwrap();
    assert_eq!(p, p2);
}
#[test]
fn basic_serialize_shape() {
    let p = Point { x: 1.5, y: -2.5 };
    let json = p.to_json_string();
    assert!(json.contains("1.5") && json.contains("-2.5"),
        "unexpected json: {}", json);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct AllPrimitives {
    a: u8,
    b: u16,
    c: u32,
    d: u64,
    e: i8,
    f: i16,
    g: i32,
    h: i64,
    fi: f32,
    fj: f64,
    flag: bool,
}
#[test]
fn all_primitives_roundtrip() {
    let v = AllPrimitives { a: 1, b: 2, c: 3, d: 4, e: -1, f: -2, g: -3, h: -4, fi: 1.5, fj: 2.5, flag: true };
    let json = v.to_json_string();
    let v2 = AllPrimitives::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct OwnedStrings {
    name: String,
    tag: String,
}
#[test]
fn owned_strings_roundtrip() {
    let v = OwnedStrings { name: "hello".into(), tag: "world".into() };
    let json = v.to_json_string();
    let v2 = OwnedStrings::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug)]
struct Borrowed<'a> {
    id:   u64,
    name: &'a str,
}
#[test]
fn zero_copy_borrow_no_allocation() {
    let input = r#"{"id":42,"name":"alice"}"#;
    let b = Borrowed::from_json_str(input).unwrap();
    assert_eq!(b.id, 42);
    assert_eq!(b.name, "alice");
    let name_start = b.name.as_ptr() as usize;
    let input_start = input.as_ptr() as usize;
    assert!(name_start >= input_start && name_start < input_start + input.len());
}
#[test]
fn escaped_string_rejects_borrow() {
    // JSON string with a \n escape — can't zero-copy borrow a &str.
    let input = "{\"id\":1,\"name\":\"ali\\nce\"}";
    let result = Borrowed::from_json_str(input);
    assert!(matches!(result, Err(jzon::Error::EscapedString)), "got: {result:?}");
}
#[test]
fn owned_string_with_escapes() {
    let v = OwnedStrings { name: "say \"hi\"".into(), tag: "line\nnewline".into() };
    let json = v.to_json_string();
    let v2 = OwnedStrings::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct CamelUser {
    user_id:   u64,
    first_name: String,
}
#[test]
fn rename_all_camel_serialize() {
    let u = CamelUser { user_id: 7, first_name: "Bob".into() };
    let json = u.to_json_string();
    assert!(json.contains("userId"), "got: {json}");
    assert!(json.contains("firstName"), "got: {json}");
}
#[test]
fn rename_all_camel_deserialize() {
    let json = r#"{"userId":7,"firstName":"Bob"}"#;
    let u = CamelUser::from_json_str(json).unwrap();
    assert_eq!(u.user_id, 7);
    assert_eq!(u.first_name, "Bob");
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct PascalPoint {
    x_coord: f64,
    y_coord: f64,
}
#[test]
fn rename_all_pascal() {
    let p = PascalPoint { x_coord: 1.0, y_coord: 2.0 };
    let json = p.to_json_string();
    assert!(json.contains("XCoord") && json.contains("YCoord"), "got: {json}");
    let p2 = PascalPoint::from_json_str(&json).unwrap();
    assert_eq!(p, p2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct ScreamPoint {
    x_coord: f64,
    y_coord: f64,
}
#[test]
fn rename_all_screaming_snake() {
    let p = ScreamPoint { x_coord: 1.0, y_coord: 2.0 };
    let json = p.to_json_string();
    assert!(json.contains("X_COORD") && json.contains("Y_COORD"), "got: {json}");
    let p2 = ScreamPoint::from_json_str(&json).unwrap();
    assert_eq!(p, p2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
struct KebabPoint {
    x_coord: f64,
    y_coord: f64,
}
#[test]
fn rename_all_kebab() {
    let p = KebabPoint { x_coord: 1.0, y_coord: 2.0 };
    let json = p.to_json_string();
    assert!(json.contains("x-coord") && json.contains("y-coord"), "got: {json}");
    let p2 = KebabPoint::from_json_str(&json).unwrap();
    assert_eq!(p, p2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Renamed {
    #[serde(rename = "fullName")]
    name: String,
    #[serde(rename = "yearsOld")]
    age: u32,
}
#[test]
fn field_rename() {
    let r = Renamed { name: "Alice".into(), age: 30 };
    let json = r.to_json_string();
    assert!(json.contains("fullName") && json.contains("yearsOld"), "got: {json}");
    let r2 = Renamed::from_json_str(&json).unwrap();
    assert_eq!(r, r2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct WithSkip {
    visible: u32,
    #[serde(skip)]
    internal: u32,
}
#[test]
fn skip_omits_field() {
    let v = WithSkip { visible: 1, internal: 999 };
    let json = v.to_json_string();
    assert!(!json.contains("internal"), "got: {json}");
    let v2 = WithSkip::from_json_str(&json).unwrap();
    assert_eq!(v2.visible, 1);
    assert_eq!(v2.internal, 0);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct SkipSer {
    value: u32,
    #[serde(skip_serializing)]
    cached: String,
}
#[test]
fn skip_serializing() {
    let v = SkipSer { value: 5, cached: "ignored".into() };
    let json = v.to_json_string();
    assert!(!json.contains("cached"), "got: {json}");
    let v2 = SkipSer::from_json_str(&json).unwrap();
    assert_eq!(v2.value, 5);
    assert_eq!(v2.cached, "");
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct SkipDe {
    value: u32,
    #[serde(skip_deserializing)]
    computed: u64,
}
#[test]
fn skip_deserializing() {
    let json = r#"{"value":3,"computed":9999}"#;
    let v = SkipDe::from_json_str(json).unwrap();
    assert_eq!(v.value, 3);
    assert_eq!(v.computed, 0);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct SkipIf {
    id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}
#[test]
fn skip_serializing_if_none() {
    let v = SkipIf { id: 1, note: None };
    let json = v.to_json_string();
    assert!(!json.contains("note"), "got: {json}");
}
#[test]
fn skip_serializing_if_some_included() {
    let v = SkipIf { id: 2, note: Some("hi".into()) };
    let json = v.to_json_string();
    assert!(json.contains("note"), "got: {json}");
    let v2 = SkipIf::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct WithDefault {
    id: u32,
    #[serde(default)]
    score: f64,
    #[serde(default = "default_tag")]
    tag: String,
}
fn default_tag() -> String { "untagged".into() }
#[test]
fn field_default_missing() {
    let json = r#"{"id":5}"#;
    let v = WithDefault::from_json_str(json).unwrap();
    assert_eq!(v.id, 5);
    assert_eq!(v.score, 0.0);
    assert_eq!(v.tag, "untagged");
}
#[test]
fn field_default_present_overrides() {
    let json = r#"{"id":5,"score":3.14,"tag":"custom"}"#;
    let v = WithDefault::from_json_str(json).unwrap();
    assert!((v.score - 3.14).abs() < 1e-9);
    assert_eq!(v.tag, "custom");
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
#[serde(default)]
struct ContainerDefault {
    x: u32,
    y: u32,
    label: String,
}
#[test]
fn container_default_all_missing() {
    let json = r#"{}"#;
    let v = ContainerDefault::from_json_str(json).unwrap();
    assert_eq!(v.x, 0);
    assert_eq!(v.y, 0);
    assert_eq!(v.label, "");
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Strict {
    id: u32,
}
#[test]
fn deny_unknown_fields_errors() {
    let json = r#"{"id":1,"extra":"value"}"#;
    let result = Strict::from_json_str(json);
    assert!(matches!(result, Err(jzon::Error::UnknownField)));
}
#[test]
fn deny_unknown_fields_ok() {
    let json = r#"{"id":1}"#;
    let v = Strict::from_json_str(json).unwrap();
    assert_eq!(v.id, 1);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct WithAlias {
    #[serde(alias = "n", alias = "full_name")]
    name: String,
}
#[test]
fn alias_primary_key() {
    let json = r#"{"name":"Alice"}"#;
    let v = WithAlias::from_json_str(json).unwrap();
    assert_eq!(v.name, "Alice");
}
#[test]
fn alias_short_key() {
    let json = r#"{"n":"Bob"}"#;
    let v = WithAlias::from_json_str(json).unwrap();
    assert_eq!(v.name, "Bob");
}
#[test]
fn alias_long_key() {
    let json = r#"{"full_name":"Carol"}"#;
    let v = WithAlias::from_json_str(json).unwrap();
    assert_eq!(v.name, "Carol");
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct WithOption {
    id: u32,
    middle_name: Option<String>,
}
#[test]
fn option_none() {
    let v = WithOption { id: 1, middle_name: None };
    let json = v.to_json_string();
    assert!(json.contains("null"), "got: {json}");
    let v2 = WithOption::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn option_some() {
    let v = WithOption { id: 2, middle_name: Some("Marie".into()) };
    let json = v.to_json_string();
    let v2 = WithOption::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn option_absent_field_is_none() {
    let json = r#"{"id":3}"#;
    let v = WithOption::from_json_str(json).unwrap();
    assert_eq!(v.middle_name, None);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct WithVec {
    tags:   Vec<String>,
    values: Vec<i32>,
}
#[test]
fn vec_roundtrip() {
    let v = WithVec { tags: vec!["a".into(), "b".into()], values: vec![1, -2, 3] };
    let json = v.to_json_string();
    let v2 = WithVec::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn vec_empty() {
    let v = WithVec { tags: vec![], values: vec![] };
    let json = v.to_json_string();
    let v2 = WithVec::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Address {
    city: String,
    zip:  String,
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Person {
    name:    String,
    age:     u8,
    address: Address,
}
#[test]
fn nested_struct_roundtrip() {
    let p = Person {
        name: "Dave".into(),
        age: 42,
        address: Address { city: "Metropolis".into(), zip: "12345".into() },
    };
    let json = p.to_json_string();
    let p2 = Person::from_json_str(&json).unwrap();
    assert_eq!(p, p2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
enum Status {
    Active,
    Inactive,
    Pending,
}
#[test]
fn unit_enum_roundtrip() {
    for s in [Status::Active, Status::Inactive, Status::Pending] {
        let json = s.to_json_string();
        let s2 = Status::from_json_str(&json).unwrap();
        assert_eq!(s, s2);
    }
}
#[test]
fn unit_enum_unknown_variant() {
    let json = r#""Unknown""#;
    assert!(matches!(Status::from_json_str(json), Err(jzon::Error::UnknownVariant)));
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
enum EventKind {
    UserSignedIn,
    UserSignedOut,
}
#[test]
fn enum_rename_all() {
    let e = EventKind::UserSignedIn;
    let json = e.to_json_string();
    assert_eq!(json, r#""user_signed_in""#);
    let e2 = EventKind::from_json_str(&json).unwrap();
    assert_eq!(e, e2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Lenient {
    id: u32,
}
#[test]
fn unknown_fields_skipped() {
    let json = r#"{"id":9,"extra":{"nested":true},"arr":[1,2,3]}"#;
    let v = Lenient::from_json_str(json).unwrap();
    assert_eq!(v.id, 9);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct MultiField {
    a: u32,
    b: u32,
    c: u32,
    d: u32,
}
#[test]
fn out_of_order_fields() {
    let json = r#"{"d":4,"b":2,"a":1,"c":3}"#;
    let v = MultiField::from_json_str(json).unwrap();
    assert_eq!(v, MultiField { a: 1, b: 2, c: 3, d: 4 });
}

// Monomorphise to a concrete type to sidestep the HRTB/lifetime interaction.
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct WrapperU32 {
    inner: u32,
    count: u32,
}
#[test]
fn generic_like_struct_u32() {
    let w = WrapperU32 { inner: 42, count: 1 };
    let json = w.to_json_string();
    let w2 = WrapperU32::from_json_str(&json).unwrap();
    assert_eq!(w, w2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Large {
    f01: u32, f02: u32, f03: u32, f04: u32, f05: u32,
    f06: u32, f07: u32, f08: u32, f09: u32, f10: u32,
}
#[test]
fn large_struct_roundtrip() {
    let v = Large { f01:1, f02:2, f03:3, f04:4, f05:5, f06:6, f07:7, f08:8, f09:9, f10:10 };
    let json = v.to_json_string();
    let v2 = Large::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson)]
struct Floats { a: f64, b: f64 }
#[test]
fn nan_infinity_to_null() {
    let v = Floats { a: f64::NAN, b: f64::INFINITY };
    let json = v.to_json_string();
    assert_eq!(json, r#"{"a":null,"b":null}"#);
}
#[test]
fn escape_roundtrip_control_chars() {
    let v = OwnedStrings {
        name: "tab\there\nnewline".into(),
        tag:  "quote\"slash\\".into(),
    };
    let json = v.to_json_string();
    let v2 = OwnedStrings::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn unicode_escape_roundtrip() {
    let v = OwnedStrings { name: "caf\u{00e9}".into(), tag: "\u{1F600}".into() };
    let json = v.to_json_string();
    let v2 = OwnedStrings::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn swar_find_quote() {
    let input = b"hello world\" rest";
    let pos = jzon::simd::find_quote_or_backslash(input, 0);
    assert_eq!(pos, 11);
}
#[test]
fn swar_find_backslash() {
    let input = b"hello\\world";
    let pos = jzon::simd::find_quote_or_backslash(input, 0);
    assert_eq!(pos, 5);
}
#[test]
fn swar_no_match_returns_len() {
    let input = b"abcdefghijklmnop";
    let pos = jzon::simd::find_quote_or_backslash(input, 0);
    assert_eq!(pos, input.len());
}
#[cfg(feature = "stats")]
#[test]
fn stats_zero_copy() {
    let input = r#"{"id":1,"name":"alice"}"#;
    let mut sc = jzon::Scanner::new_str(input);
    Borrowed::from_json_scanner(&mut sc).unwrap();
    assert_eq!(sc.stats.zero_copy_borrows, 1);
    assert_eq!(sc.stats.heap_allocations, 0);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Empty {}
#[test]
fn empty_struct_roundtrip() {
    let v = Empty {};
    let json = v.to_json_string();
    assert_eq!(json, "{}");
    let v2 = Empty::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Solo { x: u64 }
#[test]
fn single_field_roundtrip() {
    let v = Solo { x: 42 };
    let json = v.to_json_string();
    let v2 = Solo::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn integer_extremes() {
    let v = AllPrimitives { a: u8::MAX, b: u16::MAX, c: u32::MAX, d: u64::MAX,
        e: i8::MIN, f: i16::MIN, g: i32::MIN, h: i64::MIN,
        fi: 1.0, fj: 1.0, flag: false };
    let json = v.to_json_string();
    let v2 = AllPrimitives::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct L5 { val: u32, inner_a: u32, inner_b: u32 }
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct L4 { val: u32, nested: L5 }
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct L3 { val: u32, nested: L4 }
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct L2 { val: u32, nested: L3 }
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct L1 { val: u32, nested: L2 }
#[test]
fn deeply_nested_5_levels_roundtrip() {
    let v = L1 {
        val: 1,
        nested: L2 {
            val: 2,
            nested: L3 {
                val: 3,
                nested: L4 {
                    val: 4,
                    nested: L5 { val: 5, inner_a: 10, inner_b: 20 },
                },
            },
        },
    };
    let json = v.to_json_string();
    let v2 = L1::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn empty_string_roundtrip() {
    let v = OwnedStrings { name: "".into(), tag: "".into() };
    let json = v.to_json_string();
    let v2 = OwnedStrings::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn control_chars_roundtrip() {
    // Build a string from all ASCII control chars (0x00–0x1F).
    let name: String = (0u8..=31u8).map(|b| b as char).collect();
    let v = OwnedStrings { name, tag: String::new() };
    let json = v.to_json_string();
    let v2 = OwnedStrings::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn unicode_bmp_roundtrip() {
    let v = OwnedStrings {
        name: "héllo wörld 日本語".into(),
        tag: "∑∏∫".into(),
    };
    let json = v.to_json_string();
    let v2 = OwnedStrings::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn emoji_surrogate_roundtrip() {
    let v = OwnedStrings {
        name: "😀🎉🚀".into(),
        tag: "🦀".into(),
    };
    let json = v.to_json_string();
    let v2 = OwnedStrings::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson)]
struct ThreeFloats { a: f64, b: f64, c: f64 }
#[test]
fn float_special_values() {
    let v = ThreeFloats { a: f64::NAN, b: f64::INFINITY, c: f64::NEG_INFINITY };
    let json = v.to_json_string();
    assert_eq!(json, r#"{"a":null,"b":null,"c":null}"#);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Zeros { a: f64, b: f64 }
#[test]
fn float_zero_variants() {
    let v = Zeros { a: 0.0_f64, b: -0.0_f64 };
    let json = v.to_json_string();
    let v2 = Zeros::from_json_str(&json).unwrap();
    assert!((v2.a - 0.0).abs() < 1e-15);
    assert!((v2.b).abs() < 1e-15);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct BigU { v: u64 }
#[test]
fn u64_max_roundtrip() {
    let v = BigU { v: u64::MAX };
    let json = v.to_json_string();
    assert_eq!(json, r#"{"v":18446744073709551615}"#);
    let v2 = BigU::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct SignedMin { v: i64 }
#[test]
fn i64_min_roundtrip() {
    let v = SignedMin { v: i64::MIN };
    let json = v.to_json_string();
    let v2 = SignedMin::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Node {
    id: u32,
    value: f64,
    left_id: Option<u32>,
    right_id: Option<u32>,
}
#[test]
fn vec_of_nodes_roundtrip() {
    let nodes = vec![
        Node { id: 1, value: 1.5, left_id: Some(2), right_id: Some(3) },
        Node { id: 2, value: 2.5, left_id: None, right_id: None },
        Node { id: 3, value: 3.5, left_id: None, right_id: None },
    ];
    let json = nodes.to_json_string();
    let nodes2 = Vec::<Node>::from_json_str(&json).unwrap();
    assert_eq!(nodes, nodes2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct WidePhf {
    field_alpha: u64, field_beta: u64, field_gamma: u64, field_delta: u64,
    field_epsilon: u64, field_zeta: u64, field_eta: u64, field_theta: u64,
    field_iota: u64, field_kappa: u64, field_lambda: u64, field_mu: u64,
}
#[test]
fn wide_struct_phf_roundtrip() {
    let v = WidePhf {
        field_alpha: 1, field_beta: 2, field_gamma: 3, field_delta: 4,
        field_epsilon: 5, field_zeta: 6, field_eta: 7, field_theta: 8,
        field_iota: 9, field_kappa: 10, field_lambda: 11, field_mu: 12,
    };
    let json = v.to_json_string();
    let v2 = WidePhf::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn wide_struct_phf_out_of_order() {
    // All fields in reverse order — tests PHF handles arbitrary key order.
    let json = r#"{"field_mu":12,"field_lambda":11,"field_kappa":10,"field_iota":9,
                    "field_theta":8,"field_eta":7,"field_zeta":6,"field_epsilon":5,
                    "field_delta":4,"field_gamma":3,"field_beta":2,"field_alpha":1}"#;
    let v = WidePhf::from_json_str(json).unwrap();
    assert_eq!(v.field_alpha, 1);
    assert_eq!(v.field_mu, 12);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct ShortKeys { a: u64, b: u64, c: u64, id: u64, ok: bool }
#[test]
fn short_key_dispatch_roundtrip() {
    let v = ShortKeys { a: 1, b: 2, c: 3, id: 42, ok: true };
    let json = v.to_json_string();
    let v2 = ShortKeys::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct SingleChar { x: f64, y: f64, z: f64 }
#[test]
fn single_char_keys() {
    let v = SingleChar { x: 1.0, y: 2.0, z: 3.0 };
    let json = v.to_json_string();
    let v2 = SingleChar::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn serialization_uses_size_hint() {
    let v = Point { x: 1.5, y: 2.5 };
    let bytes = v.to_json_bytes();
    assert!(
        bytes.capacity() <= bytes.len() * 4,
        "capacity {} >> len {}",
        bytes.capacity(),
        bytes.len()
    );
}
#[test]
fn deny_unknown_nested_value_skipped() {
    let json = r#"{"id":9,"nested":{"a":1,"b":[1,2,3]},"another":null}"#;
    let v = Lenient::from_json_str(json).unwrap();
    assert_eq!(v.id, 9);
}
#[test]
fn swar_finds_at_all_offsets_mod_8() {
    for offset in 0usize..16 {
        let mut input = vec![b'x'; offset + 10];
        input[offset] = b'"';
        let pos = jzon::simd::find_quote_or_backslash(&input, 0);
        assert_eq!(pos, offset, "SWAR failed at offset {}", offset);
    }
}
#[test]
fn swar_finds_backslash_at_all_offsets() {
    for offset in 0usize..16 {
        let mut input = vec![b'a'; offset + 10];
        input[offset] = b'\\';
        let pos = jzon::simd::find_quote_or_backslash(&input, 0);
        assert_eq!(pos, offset, "SWAR backslash failed at offset {}", offset);
    }
}
#[derive(ToJson, FromJson, Debug, PartialEq)]
struct Xyz { x: f64, y: f64, z: f64 }
#[test]
fn fused_key_colon_with_spaces() {
    let json = "{\n  \"x\" : 1.0 ,\n  \"y\" : -2.5 ,\n  \"z\" : 3.14\n}";
    let v = Xyz::from_json_str(json).unwrap();
    assert!((v.x - 1.0).abs() < 1e-12);
    assert!((v.y - (-2.5)).abs() < 1e-12);
    assert!((v.z - 3.14).abs() < 1e-10);
}
#[test]
fn capacity_hint_reasonable() {
    let v = Point { x: 1.5, y: 2.5 };
    let bytes = v.to_json_bytes();
    assert!(bytes.capacity() <= bytes.len() * 4,
        "excessive pre-allocation: cap={} len={}", bytes.capacity(), bytes.len());
}
#[test]
fn unknown_nested_object_skipped() {
    let json = r#"{"id":9,"nested":{"a":1,"b":[1,2,3]},"another":null,"arr":[{"x":1}]}"#;
    let v = Lenient::from_json_str(json).unwrap();
    assert_eq!(v.id, 9);
}
#[test]
fn fixed_buf_stack_roundtrip() {
    use jzon::ToJsonExt;
    let p = Point { x: 1.5, y: -2.0 };
    let buf = p.to_fixed_buf::<64>().expect("64 bytes is enough for Point");
    assert!(buf.len() > 0);
    assert!(buf.as_str().contains("1.5"));
    let p2 = Point::from_json_bytes(buf.as_slice()).unwrap();
    assert_eq!(p.x, p2.x);
}
#[test]
fn json_write_reuse_deterministic() {
    use jzon::ToJsonExt;
    let p = Point { x: 3.0, y: 4.0 };
    let mut buf = Vec::with_capacity(64);
    let out1 = p.json_write_reuse(&mut buf).to_vec();
    let out2 = p.json_write_reuse(&mut buf).to_vec();
    assert_eq!(out1, out2);
}
#[test]
fn json_sink_vec_and_fixed_produce_same_output() {
    use jzon::ToJsonExt;
    let p = Point { x: 1.5, y: -2.0 };
    let vec_out = p.to_json_bytes();
    let fixed_out = p.to_fixed_buf::<64>().unwrap();
    assert_eq!(vec_out.as_slice(), fixed_out.as_slice());
}
#[test]
fn const_json_str_len_correctness() {
    use jzon::json_str_len;
    assert_eq!(json_str_len(b"hello"), 7);
    assert_eq!(json_str_len(b"say\n"), 7);
    assert_eq!(json_str_len(b""), 2);
}
#[test]
fn jzon_serde_roundtrip_matches_serde_json() {
    #[derive(jzon::ToJson, jzon::FromJson, serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct Payload { id: u64, value: f64, tag: String }

    let p = Payload { id: 42, value: 3.14, tag: "hello".into() };
    let rjson_out = p.to_json_string();
    let serde_out = serde_json::to_string(&p).unwrap();
    assert_eq!(rjson_out, serde_out, "Mode A output must match serde_json");

    let mode_b_out = jzon_serde::to_string(&p).unwrap();
    assert_eq!(mode_b_out, serde_out, "Mode B output must match serde_json");
}
#[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
struct WithChar { c: char }
#[test]
fn char_roundtrip() {
    let v = WithChar { c: 'A' };
    let json = v.to_json_string();
    assert!(json.contains("\"A\""), "expected serialized char, got: {json}");
    let v2 = WithChar::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn char_multibyte_roundtrip() {
    let v = WithChar { c: '€' };
    let json = v.to_json_string();
    let v2 = WithChar::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(jzon::ToJson)]
struct UnitNewtype;
#[test]
fn unit_struct_serializes_braces() {
    let v = UnitNewtype;
    let json = v.to_json_string();
    assert_eq!(json, "{}", "unit struct should serialize as empty object, got: {json}");
}
#[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
struct Miles(f64);
#[test]
fn newtype_struct_delegates_to_inner() {
    let v = Miles(3.14);
    let json = v.to_json_string();
    assert!(!json.contains('{'), "newtype should not add braces, got: {json}");
    let v2 = Miles::from_json_str(&json).unwrap();
    assert!((v2.0 - 3.14).abs() < 1e-10, "roundtrip mismatch: {}", v2.0);
}
#[test]
fn newtype_struct_integer() {
    #[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
    struct UserId(u64);
    let v = UserId(42);
    let json = v.to_json_string();
    assert_eq!(json, "42");
    let v2 = UserId::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
struct Pair(u64, f64);
#[test]
fn tuple_struct_serializes_as_array() {
    let v = Pair(42, 3.14);
    let json = v.to_json_string();
    assert!(json.starts_with('['), "tuple struct should be JSON array, got: {json}");
    assert!(json.contains("42"), "got: {json}");
    let v2 = Pair::from_json_str(&json).unwrap();
    assert_eq!(v.0, v2.0);
    assert!((v.1 - v2.1).abs() < 1e-10);
}
#[test]
fn tuple_struct_three_fields() {
    #[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
    struct Triple(i32, String, bool);
    let v = Triple(-1, "hi".into(), true);
    let json = v.to_json_string();
    assert!(json.starts_with('['), "got: {json}");
    let v2 = Triple::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
#[serde(transparent)]
struct Wrapper { inner: f64 }
#[test]
fn transparent_delegates() {
    let v = Wrapper { inner: 2.718 };
    let json = v.to_json_string();
    assert!(!json.contains('{'), "transparent should not wrap in object, got: {json}");
    let v2 = Wrapper::from_json_str(&json).unwrap();
    assert!((v2.inner - 2.718).abs() < 1e-10, "roundtrip mismatch: {}", v2.inner);
}
#[test]
fn transparent_string_delegates() {
    #[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
    #[serde(transparent)]
    struct Tag { value: String }
    let v = Tag { value: "hello".into() };
    let json = v.to_json_string();
    assert_eq!(json, r#""hello""#, "transparent String should serialize as bare string");
    let v2 = Tag::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn tuple_two_roundtrip() {
    let v: (u64, String) = (42, "hello".into());
    let json = v.to_json_string();
    assert_eq!(json, r#"[42,"hello"]"#, "2-tuple should serialize as JSON array");
    let v2: (u64, String) = FromJson::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn tuple_three_roundtrip() {
    let v: (i32, f64, bool) = (-1, 2.5, true);
    let json = v.to_json_string();
    assert!(json.starts_with('['), "3-tuple should be array, got: {json}");
    let v2: (i32, f64, bool) = FromJson::from_json_str(&json).unwrap();
    assert_eq!(v.0, v2.0);
    assert!((v.1 - v2.1).abs() < 1e-10);
    assert_eq!(v.2, v2.2);
}
#[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
struct BigInts { v_u128: u128, v_i128: i128 }
#[test]
fn u128_large_value() {
    let v = BigInts { v_u128: u128::MAX, v_i128: i128::MIN };
    let json = v.to_json_string();
    let v2 = BigInts::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn u128_zero() {
    let v = BigInts { v_u128: 0, v_i128: 0 };
    let json = v.to_json_string();
    let v2 = BigInts::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn vec_u8_is_array() {
    let v: Vec<u8> = vec![1, 2, 255];
    let json = v.to_json_string();
    assert_eq!(json, "[1,2,255]", "Vec<u8> must serialize as integer array");
}
#[test]
fn vec_u8_empty() {
    let v: Vec<u8> = vec![];
    let json = v.to_json_string();
    assert_eq!(json, "[]");
}
#[test]
fn hashmap_to_json() {
    use std::collections::HashMap;
    let mut m: HashMap<String, u64> = HashMap::new();
    m.insert("a".into(), 1);
    let json = m.to_json_string();
    assert!(json.starts_with('{'), "HashMap should serialize as JSON object, got: {json}");
    assert!(json.contains("\"a\""), "got: {json}");
    assert!(json.contains(':'), "got: {json}");
}
#[test]
fn hashmap_single_entry_roundtrip() {
    use std::collections::HashMap;
    let mut m: HashMap<String, String> = HashMap::new();
    m.insert("key".into(), "val".into());
    let json = m.to_json_string();
    let m2: HashMap<String, String> = FromJson::from_json_str(&json).unwrap();
    assert_eq!(m2.get("key").map(String::as_str), Some("val"));
}
#[test]
fn hashmap_from_json() {
    use std::collections::HashMap;
    let json = r#"{"x":1,"y":2,"z":3}"#;
    let m: HashMap<String, u64> = FromJson::from_json_str(json).unwrap();
    assert_eq!(m.get("x").copied(), Some(1));
    assert_eq!(m.get("z").copied(), Some(3));
    assert_eq!(m.len(), 3);
}
#[test]
fn hashmap_empty_object() {
    use std::collections::HashMap;
    let json = r#"{}"#;
    let m: HashMap<String, u64> = FromJson::from_json_str(json).unwrap();
    assert!(m.is_empty());
}
#[derive(jzon::ToJson)]
#[serde(tag = "type")]
enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}
#[test]
fn enum_struct_variant_ser_circle() {
    let c = Shape::Circle { radius: 1.5 };
    let json = c.to_json_string();
    assert!(json.contains("\"type\""), "internally tagged: missing 'type' key, got: {json}");
    assert!(json.contains("\"Circle\""), "expected variant name, got: {json}");
    assert!(json.contains("1.5"), "expected radius, got: {json}");
}
#[test]
fn enum_struct_variant_ser_rectangle() {
    let r = Shape::Rectangle { width: 2.0, height: 3.0 };
    let json = r.to_json_string();
    assert!(json.contains("\"Rectangle\""), "expected variant name, got: {json}");
    assert!(json.contains("\"width\""), "expected width field, got: {json}");
    assert!(json.contains("\"height\""), "expected height field, got: {json}");
}
#[test]
fn btreemap_roundtrip() {
    use std::collections::BTreeMap;
    let mut m: BTreeMap<String, i64> = BTreeMap::new();
    m.insert("alpha".into(), -1);
    m.insert("beta".into(), 2);
    m.insert("gamma".into(), 0);
    let json = m.to_json_string();
    let m2: BTreeMap<String, i64> = FromJson::from_json_str(&json).unwrap();
    assert_eq!(m, m2);
}
#[test]
fn btreemap_sorted_key_order() {
    use std::collections::BTreeMap;
    let mut m: BTreeMap<String, u64> = BTreeMap::new();
    m.insert("z".into(), 3);
    m.insert("a".into(), 1);
    m.insert("m".into(), 2);
    let json = m.to_json_string();
    let a_pos = json.find("\"a\"").unwrap_or(usize::MAX);
    let m_pos = json.find("\"m\"").unwrap_or(usize::MAX);
    let z_pos = json.find("\"z\"").unwrap_or(usize::MAX);
    assert!(a_pos < m_pos && m_pos < z_pos, "BTreeMap not sorted: {json}");
}
#[test]
fn char_via_serde_compat() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct S { c: char }
    let v = S { c: '€' };
    let json = jzon_serde::to_string(&v).unwrap();
    let v2: S = jzon_serde::from_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
struct ScoreEntry { score: u64, rank: u32 }
#[test]
fn hashmap_struct_value_roundtrip() {
    use std::collections::HashMap;
    let mut m: HashMap<String, ScoreEntry> = HashMap::new();
    m.insert("alice".into(), ScoreEntry { score: 100, rank: 1 });
    m.insert("bob".into(), ScoreEntry { score: 80, rank: 2 });
    let json = m.to_json_string();
    let m2: HashMap<String, ScoreEntry> = FromJson::from_json_str(&json).unwrap();
    assert_eq!(m2.get("alice"), Some(&ScoreEntry { score: 100, rank: 1 }));
    assert_eq!(m2.len(), 2);
}
#[test]
fn vec_of_tuples_roundtrip() {
    let v: Vec<(u64, String)> = vec![(1, "one".into()), (2, "two".into()), (3, "three".into())];
    let json = v.to_json_string();
    let v2: Vec<(u64, String)> = FromJson::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[test]
fn empty_tuple_struct_is_empty_array() {
    #[derive(jzon::ToJson, jzon::FromJson, Debug, PartialEq)]
    struct EmptyTuple();
    let v = EmptyTuple();
    let json = v.to_json_string();
    assert_eq!(json, "[]", "empty tuple struct should be empty JSON array");
    let v2 = EmptyTuple::from_json_str(&json).unwrap();
    assert_eq!(v, v2);
}
#[derive(jzon::ToJson, jzon::FromJson, serde::Serialize, serde::Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
enum TaggedShape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}
#[test]
fn internally_tagged_enum_roundtrip() {
    let c = TaggedShape::Circle { radius: 1.5 };
    let json = c.to_json_string();
    assert!(json.contains("\"Circle\"") && json.contains("1.5"), "got: {json}");
    let c2 = TaggedShape::from_json_str(&json).unwrap();
    assert_eq!(c, c2);

    let r = TaggedShape::Rectangle { width: 2.0, height: 3.0 };
    let rjson = r.to_json_string();
    let r2 = TaggedShape::from_json_str(&rjson).unwrap();
    assert_eq!(r, r2);
}
#[test]
fn internally_tagged_enum_tag_not_first() {
    // Tag appears AFTER other fields — two-pass approach handles this correctly.
    let json = r#"{"width":4.0,"height":5.0,"type":"Rectangle"}"#;
    let r = TaggedShape::from_json_str(json).unwrap();
    assert_eq!(r, TaggedShape::Rectangle { width: 4.0, height: 5.0 });
}
#[test]
fn internally_tagged_enum_matches_serde_json() {
    let c = TaggedShape::Circle { radius: 2.7 };
    let jzon_out = c.to_json_string();
    let serde_out = serde_json::to_string(&c).unwrap();
    assert_eq!(jzon_out, serde_out, "jzon output must match serde_json");
}
