//! Serde compatibility test suite for jzon_serde.
//!
//! Tests are adapted from serde_json's own test suite
//! (~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/serde_json-1.0.117/tests/test.rs)
//! to verify that jzon_serde handles the same serde usage patterns.
//!
//! All tests pass — including `#[serde(flatten)]`, `#[serde(untagged)]`, and
//! multi-field tuple enum variants which required a `JsonSeqAccess` drain-on-drop
//! fix and a `skip_array_tail` addition to the scanner.
//!
//! Run with: cargo test -p jzon-rs --test serde_compat

#![allow(clippy::derive_partial_eq_without_eq)]

use jzon_serde::{from_slice, from_str, to_string};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

// ---- helper ----------------------------------------------------------------

fn roundtrip<T>(val: &T) -> T
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let json = to_string(val).expect("serialize");
    from_str(&json).expect("deserialize")
}

// ============================================================
// 1. PRIMITIVE TYPES — serialization
// ============================================================

#[test]
fn ser_null() {
    assert_eq!(to_string(&()).unwrap(), "null");
}

#[test]
fn ser_bool() {
    assert_eq!(to_string(&true).unwrap(), "true");
    assert_eq!(to_string(&false).unwrap(), "false");
}

#[test]
fn ser_u64() {
    assert_eq!(to_string(&3u64).unwrap(), "3");
    assert_eq!(to_string(&u64::MAX).unwrap(), u64::MAX.to_string());
}

#[test]
fn ser_i64() {
    assert_eq!(to_string(&3i64).unwrap(), "3");
    assert_eq!(to_string(&-2i64).unwrap(), "-2");
    assert_eq!(to_string(&-1234i64).unwrap(), "-1234");
    assert_eq!(to_string(&i64::MIN).unwrap(), i64::MIN.to_string());
}

#[test]
fn ser_f64() {
    // jzon_serde now emits "3.0" for whole-number floats, matching serde_json.
    assert_eq!(to_string(&3.0f64).unwrap(), "3.0");
    assert_eq!(to_string(&3.1f64).unwrap(), "3.1");
    assert_eq!(to_string(&-1.5f64).unwrap(), "-1.5");
    assert_eq!(to_string(&0.5f64).unwrap(), "0.5");
}

#[test]
fn ser_string() {
    assert_eq!(to_string(&"").unwrap(), "\"\"");
    assert_eq!(to_string(&"foo").unwrap(), "\"foo\"");
}

#[test]
fn ser_string_escapes() {
    // quote, backslash, control chars
    assert_eq!(to_string(&"\"").unwrap(), "\"\\\"\"");
    assert_eq!(to_string(&"\\").unwrap(), "\"\\\\\"");
    assert_eq!(to_string(&"\n").unwrap(), "\"\\n\"");
    assert_eq!(to_string(&"\r").unwrap(), "\"\\r\"");
    assert_eq!(to_string(&"\t").unwrap(), "\"\\t\"");
}

#[test]
fn ser_option_some_and_none() {
    let none: Option<String> = None;
    assert_eq!(to_string(&none).unwrap(), "null");
    assert_eq!(to_string(&Some("jodhpurs")).unwrap(), "\"jodhpurs\"");
}

#[test]
fn ser_vec() {
    assert_eq!(to_string(&Vec::<bool>::new()).unwrap(), "[]");
    assert_eq!(to_string(&vec![true]).unwrap(), "[true]");
    assert_eq!(to_string(&vec![true, false]).unwrap(), "[true,false]");
}

#[test]
fn ser_tuple() {
    assert_eq!(to_string(&(5u32,)).unwrap(), "[5]");
    assert_eq!(to_string(&(5u32, 6u32, "abc")).unwrap(), "[5,6,\"abc\"]");
}

// ============================================================
// 2. PRIMITIVE TYPES — deserialization
// ============================================================

#[test]
fn de_null() {
    let v: () = from_str("null").unwrap();
    assert_eq!(v, ());
}

#[test]
fn de_bool() {
    assert_eq!(from_str::<bool>("true").unwrap(), true);
    assert_eq!(from_str::<bool>(" true ").unwrap(), true);
    assert_eq!(from_str::<bool>("false").unwrap(), false);
    assert_eq!(from_str::<bool>(" false ").unwrap(), false);
}

#[test]
fn de_i64() {
    assert_eq!(from_str::<i64>("-2").unwrap(), -2);
    assert_eq!(from_str::<i64>("-1234").unwrap(), -1234);
    assert_eq!(from_str::<i64>(" -1234 ").unwrap(), -1234);
    assert_eq!(from_str::<i64>(&i64::MIN.to_string()).unwrap(), i64::MIN);
    assert_eq!(from_str::<i64>(&i64::MAX.to_string()).unwrap(), i64::MAX);
}

#[test]
fn de_u64() {
    assert_eq!(from_str::<u64>("0").unwrap(), 0);
    assert_eq!(from_str::<u64>("3").unwrap(), 3);
    assert_eq!(from_str::<u64>("1234").unwrap(), 1234);
    assert_eq!(from_str::<u64>(&u64::MAX.to_string()).unwrap(), u64::MAX);
}

#[test]
fn de_f64() {
    assert!((from_str::<f64>("0.0").unwrap() - 0.0f64).abs() < f64::EPSILON);
    assert!((from_str::<f64>("3.1").unwrap() - 3.1f64).abs() < 1e-10);
    assert!((from_str::<f64>("-1.2").unwrap() - (-1.2f64)).abs() < 1e-10);
    // scientific notation
    assert!((from_str::<f64>("0.4e5").unwrap() - 0.4e5).abs() < 1.0);
    assert!((from_str::<f64>("0.4e-01").unwrap() - 0.04f64).abs() < 1e-10);
}

#[test]
fn de_string() {
    assert_eq!(from_str::<String>("\"\"").unwrap(), "");
    assert_eq!(from_str::<String>("\"foo\"").unwrap(), "foo");
    assert_eq!(from_str::<String>(" \"foo\" ").unwrap(), "foo");
    assert_eq!(from_str::<String>("\"\\\"\"").unwrap(), "\"");
    assert_eq!(from_str::<String>("\"\\n\"").unwrap(), "\n");
    assert_eq!(from_str::<String>("\"\\r\"").unwrap(), "\r");
    assert_eq!(from_str::<String>("\"\\t\"").unwrap(), "\t");
}

#[test]
fn de_string_unicode_escape() {
    assert_eq!(from_str::<String>("\"\\u12ab\"").unwrap(), "\u{12ab}");
    assert_eq!(from_str::<String>("\"\\uAB12\"").unwrap(), "\u{AB12}");
    // surrogate pair
    assert_eq!(from_str::<String>("\"\\uD83C\\uDF95\"").unwrap(), "\u{1F395}");
}

#[test]
fn de_vec() {
    assert_eq!(from_str::<Vec<u64>>("[]").unwrap(), vec![]);
    assert_eq!(from_str::<Vec<u64>>("[ ]").unwrap(), vec![]);
    assert_eq!(from_str::<Vec<u64>>("[3,1]").unwrap(), vec![3, 1]);
    assert_eq!(from_str::<Vec<u64>>(" [ 3 , 1 ] ").unwrap(), vec![3, 1]);
}

#[test]
fn de_option() {
    assert_eq!(from_str::<Option<String>>("null").unwrap(), None);
    assert_eq!(
        from_str::<Option<String>>("\"jodhpurs\"").unwrap(),
        Some("jodhpurs".to_string())
    );
}

#[test]
fn de_btreemap() {
    let result: BTreeMap<String, u64> = from_str("{\"a\":3,\"b\":4}").unwrap();
    assert_eq!(result["a"], 3);
    assert_eq!(result["b"], 4);
}

#[test]
fn de_from_slice() {
    let v: Vec<u32> = from_slice(b"[1,2,3]").unwrap();
    assert_eq!(v, vec![1u32, 2, 3]);
}

#[test]
fn de_trailing_whitespace() {
    assert_eq!(from_str::<Vec<u64>>("[1, 2] ").unwrap(), vec![1, 2]);
    assert_eq!(from_str::<Vec<u64>>("[1, 2]\n").unwrap(), vec![1, 2]);
    assert_eq!(from_str::<Vec<u64>>("[1, 2]\t").unwrap(), vec![1, 2]);
}

// ============================================================
// 3. BASIC STRUCT ROUNDTRIP
// ============================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Inner {
    a: (),
    b: usize,
    c: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Outer {
    inner: Vec<Inner>,
}

#[test]
fn roundtrip_nested_structs() {
    let v = Outer {
        inner: vec![Inner {
            a: (),
            b: 2,
            c: vec!["abc".to_string(), "xyz".to_string()],
        }],
    };
    assert_eq!(roundtrip(&v), v);
}

#[test]
fn de_struct_from_json() {
    let json = r#"{"inner":[{"a":null,"b":2,"c":["abc","xyz"]}]}"#;
    let v: Outer = from_str(json).unwrap();
    assert_eq!(
        v,
        Outer {
            inner: vec![Inner {
                a: (),
                b: 2,
                c: vec!["abc".to_string(), "xyz".to_string()],
            }]
        }
    );
}

#[test]
fn de_struct_missing_required_field() {
    // `inner` is required; empty object should fail
    let err = from_str::<Outer>("{}").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("missing field"),
        "expected 'missing field' in: {msg}"
    );
}

// ============================================================
// 4. ENUM VARIANTS
// ============================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Animal {
    Dog,
    Frog(String, Vec<isize>),
    Cat { age: usize, name: String },
    AntHive(Vec<String>),
}

#[test]
fn ser_enum_unit_variant() {
    assert_eq!(to_string(&Animal::Dog).unwrap(), "\"Dog\"");
}

#[test]
fn ser_enum_tuple_variant() {
    assert_eq!(
        to_string(&Animal::Frog("Henry".to_string(), vec![])).unwrap(),
        r#"{"Frog":["Henry",[]]}"#
    );
    assert_eq!(
        to_string(&Animal::Frog("Henry".to_string(), vec![349, 102])).unwrap(),
        r#"{"Frog":["Henry",[349,102]]}"#
    );
}

#[test]
fn ser_enum_struct_variant() {
    assert_eq!(
        to_string(&Animal::Cat {
            age: 5,
            name: "Kate".to_string(),
        })
        .unwrap(),
        r#"{"Cat":{"age":5,"name":"Kate"}}"#
    );
}

#[test]
fn ser_enum_newtype_vec_variant() {
    assert_eq!(
        to_string(&Animal::AntHive(vec!["Bob".to_string(), "Stuart".to_string()])).unwrap(),
        r#"{"AntHive":["Bob","Stuart"]}"#
    );
}

#[test]
fn de_enum_unit_variant() {
    assert_eq!(from_str::<Animal>("\"Dog\"").unwrap(), Animal::Dog);
    assert_eq!(from_str::<Animal>(" \"Dog\" ").unwrap(), Animal::Dog);
}

#[test]
fn de_enum_tuple_variant() {
    assert_eq!(
        from_str::<Animal>(r#"{"Frog":["Henry",[]]}"#).unwrap(),
        Animal::Frog("Henry".to_string(), vec![])
    );
    assert_eq!(
        from_str::<Animal>(r#" { "Frog": [ "Henry" , [ 349, 102 ] ] } "#).unwrap(),
        Animal::Frog("Henry".to_string(), vec![349, 102])
    );
}

#[test]
fn de_enum_struct_variant() {
    assert_eq!(
        from_str::<Animal>(r#"{"Cat":{"age":5,"name":"Kate"}}"#).unwrap(),
        Animal::Cat { age: 5, name: "Kate".to_string() }
    );
}

#[test]
fn de_enum_unknown_variant_error() {
    let err = from_str::<Animal>("\"unknown\"").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown variant") || msg.contains("unknown"),
        "expected 'unknown variant' in: {msg}"
    );
}

#[test]
fn de_enum_deny_unknown_fields() {
    // Cat has deny_unknown_fields via the Animal enum
    let err =
        from_str::<Animal>(r#"{"Cat":{"age":5,"name":"Kate","foo":"bar"}}"#).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field") || msg.contains("unknown"),
        "expected 'unknown field' error in: {msg}"
    );
}

#[test]
fn roundtrip_animal_supported_variants() {
    // Dog (unit), Cat (struct variant), AntHive (newtype Vec) all work.
    // Frog (multi-field tuple variant) is ignored separately.
    for animal in &[
        Animal::Dog,
        Animal::Cat { age: 5, name: "Kate".to_string() },
        Animal::AntHive(vec!["Bob".to_string()]),
    ] {
        assert_eq!(&roundtrip(animal), animal);
    }
}

#[test]
fn roundtrip_all_animal_variants() {
    for animal in &[
        Animal::Dog,
        Animal::Frog("Henry".to_string(), vec![1, 2, 3]),
        Animal::Cat { age: 5, name: "Kate".to_string() },
        Animal::AntHive(vec!["Bob".to_string()]),
    ] {
        assert_eq!(&roundtrip(animal), animal);
    }
}

// ============================================================
// 5. SERDE ATTRIBUTES — rename, rename_all, skip, default, alias
// ============================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Renamed {
    #[serde(rename = "full_name")]
    name: String,
    age: u32,
}

#[test]
fn serde_attr_rename_ser() {
    let r = Renamed { name: "Alice".to_string(), age: 30 };
    let json = to_string(&r).unwrap();
    assert!(json.contains("\"full_name\""), "expected 'full_name' in: {json}");
    assert!(!json.contains("\"name\""), "unexpected 'name' key in: {json}");
}

#[test]
fn serde_attr_rename_de() {
    let r: Renamed = from_str(r#"{"full_name":"Bob","age":25}"#).unwrap();
    assert_eq!(r.name, "Bob");
    assert_eq!(r.age, 25);
}

#[test]
fn serde_attr_rename_missing_field_uses_renamed_key() {
    // using the original field name "name" should fail when rename = "full_name"
    let err = from_str::<Renamed>(r#"{"name":"Bob","age":25}"#).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("missing field") || msg.contains("unknown field"),
        "expected field error in: {msg}"
    );
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CamelCase {
    first_name: String,
    last_name: String,
}

#[test]
fn serde_attr_rename_all_camel_case_ser() {
    let v = CamelCase { first_name: "John".to_string(), last_name: "Doe".to_string() };
    let json = to_string(&v).unwrap();
    assert!(json.contains("\"firstName\""), "expected 'firstName' in: {json}");
    assert!(json.contains("\"lastName\""), "expected 'lastName' in: {json}");
}

#[test]
fn serde_attr_rename_all_camel_case_de() {
    let v: CamelCase = from_str(r#"{"firstName":"John","lastName":"Doe"}"#).unwrap();
    assert_eq!(v.first_name, "John");
    assert_eq!(v.last_name, "Doe");
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct WithSkip {
    kept: u32,
    #[serde(skip)]
    skipped: u32,
}

#[test]
fn serde_attr_skip_in_ser() {
    let v = WithSkip { kept: 1, skipped: 99 };
    let json = to_string(&v).unwrap();
    assert!(!json.contains("skipped"), "skipped field must not appear in: {json}");
    assert!(json.contains("\"kept\""), "kept field must appear in: {json}");
}

#[test]
fn serde_attr_skip_in_de_uses_default() {
    let v: WithSkip = from_str(r#"{"kept":42}"#).unwrap();
    assert_eq!(v.kept, 42);
    assert_eq!(v.skipped, 0); // Default::default()
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct WithSkipSerIf {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    nickname: Option<String>,
}

#[test]
fn serde_attr_skip_serializing_if_none() {
    let v = WithSkipSerIf { name: "Alice".to_string(), nickname: None };
    let json = to_string(&v).unwrap();
    assert!(!json.contains("nickname"), "nickname must be absent when None: {json}");
}

#[test]
fn serde_attr_skip_serializing_if_some() {
    let v = WithSkipSerIf {
        name: "Alice".to_string(),
        nickname: Some("Ali".to_string()),
    };
    let json = to_string(&v).unwrap();
    assert!(json.contains("\"nickname\""), "nickname must be present when Some: {json}");
    assert!(json.contains("\"Ali\""), "nickname value must be present: {json}");
}

#[derive(Debug, PartialEq, Deserialize)]
struct WithAlias {
    #[serde(alias = "full_name", alias = "name")]
    username: String,
}

#[test]
fn serde_attr_alias_primary() {
    let v: WithAlias = from_str(r#"{"username":"alice"}"#).unwrap();
    assert_eq!(v.username, "alice");
}

#[test]
fn serde_attr_alias_first_alias() {
    let v: WithAlias = from_str(r#"{"full_name":"alice"}"#).unwrap();
    assert_eq!(v.username, "alice");
}

#[test]
fn serde_attr_alias_second_alias() {
    let v: WithAlias = from_str(r#"{"name":"alice"}"#).unwrap();
    assert_eq!(v.username, "alice");
}

#[derive(Debug, PartialEq, Deserialize)]
struct WithDefault {
    #[serde(default)]
    count: u32,
    label: String,
}

#[test]
fn serde_attr_default_missing_uses_default() {
    let v: WithDefault = from_str(r#"{"label":"hello"}"#).unwrap();
    assert_eq!(v.count, 0);
    assert_eq!(v.label, "hello");
}

#[test]
fn serde_attr_default_present_uses_value() {
    let v: WithDefault = from_str(r#"{"count":5,"label":"hello"}"#).unwrap();
    assert_eq!(v.count, 5);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Strict {
    x: i32,
    y: i32,
}

#[test]
fn serde_attr_deny_unknown_fields_ok() {
    let v: Strict = from_str(r#"{"x":1,"y":2}"#).unwrap();
    assert_eq!(v, Strict { x: 1, y: 2 });
}

#[test]
fn serde_attr_deny_unknown_fields_error() {
    let err = from_str::<Strict>(r#"{"x":1,"y":2,"z":3}"#).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field"),
        "expected 'unknown field' error in: {msg}"
    );
}

// ============================================================
// 6. INTERNALLY TAGGED ENUMS
// ============================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}

#[test]
fn internally_tagged_ser_circle() {
    let s = Shape::Circle { radius: 2.5 };
    let json = to_string(&s).unwrap();
    assert!(json.contains("\"type\""), "must contain type tag: {json}");
    assert!(json.contains("\"Circle\""), "must contain variant name: {json}");
    assert!(json.contains("\"radius\""), "must contain field: {json}");
}

#[test]
fn internally_tagged_de_circle() {
    let s: Shape = from_str(r#"{"type":"Circle","radius":2.5}"#).unwrap();
    assert_eq!(s, Shape::Circle { radius: 2.5 });
}

#[test]
fn internally_tagged_de_rectangle() {
    let s: Shape = from_str(r#"{"type":"Rectangle","width":3.0,"height":4.0}"#).unwrap();
    assert_eq!(s, Shape::Rectangle { width: 3.0, height: 4.0 });
}

#[test]
fn internally_tagged_roundtrip() {
    for shape in &[
        Shape::Circle { radius: 1.0 },
        Shape::Rectangle { width: 2.0, height: 3.0 },
    ] {
        assert_eq!(&roundtrip(shape), shape);
    }
}

// ============================================================
// 7. EDGE CASES
// ============================================================

#[test]
fn edge_empty_string_roundtrip() {
    let s = String::new();
    assert_eq!(roundtrip(&s), s);
}

#[test]
fn edge_unicode_roundtrip() {
    let s = "Ελληνικά 日本語 🎕".to_string();
    assert_eq!(roundtrip(&s), s);
}

#[test]
fn edge_null_roundtrip() {
    let v: Option<u32> = None;
    let json = to_string(&v).unwrap();
    assert_eq!(json, "null");
    let v2: Option<u32> = from_str(&json).unwrap();
    assert_eq!(v2, None);
}

#[test]
fn edge_large_number_roundtrip() {
    let n = u64::MAX;
    assert_eq!(roundtrip(&n), n);
    let n = i64::MIN;
    assert_eq!(roundtrip(&n), n);
}

#[test]
fn edge_vec_of_options() {
    let v: Vec<Option<u32>> = vec![Some(1), None, Some(3)];
    let json = to_string(&v).unwrap();
    assert_eq!(json, "[1,null,3]");
    let v2: Vec<Option<u32>> = from_str(&json).unwrap();
    assert_eq!(v, v2);
}

#[test]
fn edge_nested_option() {
    let v: Option<Option<u32>> = Some(Some(42));
    let json = to_string(&v).unwrap();
    assert_eq!(json, "42");
    let v2: Option<Option<u32>> = from_str(&json).unwrap();
    assert_eq!(v, v2);
}

#[test]
fn edge_special_float_nan_serializes_as_null() {
    // jzon_serde serializes NaN/Inf as null (same as serde_json behaviour)
    let json = to_string(&f64::NAN).unwrap();
    assert_eq!(json, "null");
}

#[test]
fn edge_special_float_inf_serializes_as_null() {
    let json = to_string(&f64::INFINITY).unwrap();
    assert_eq!(json, "null");
    let json = to_string(&f64::NEG_INFINITY).unwrap();
    assert_eq!(json, "null");
}

#[test]
fn edge_special_float_f32_nan_serializes_as_null() {
    let json = to_string(&f32::NAN).unwrap();
    assert_eq!(json, "null");
}

// ============================================================
// 8. ERROR CASES
// ============================================================

#[test]
fn error_wrong_type_for_bool() {
    let err = from_str::<bool>("42").unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn error_wrong_type_for_struct() {
    let err = from_str::<Outer>("5").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("invalid type") || msg.contains("expected"),
        "expected type error in: {msg}"
    );
}

#[test]
fn error_missing_required_field() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Foo {
        x: u32,
    }
    let err = from_str::<Foo>("{}").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("missing field"), "expected 'missing field' in: {msg}");
}

#[test]
fn error_unknown_field_with_deny() {
    let err = from_str::<Strict>(r#"{"x":1,"y":2,"extra":true}"#).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown field"), "expected 'unknown field' in: {msg}");
}

#[test]
fn error_invalid_json_syntax() {
    assert!(from_str::<u32>("{").is_err());
    assert!(from_str::<u32>("[").is_err());
    assert!(from_str::<u32>("").is_err());
}

#[test]
fn error_unknown_enum_variant() {
    let err = from_str::<Animal>(r#"{"Parrot":null}"#).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown") || msg.contains("variant"),
        "expected variant error in: {msg}"
    );
}

// ============================================================
// 9. COMPLEX NESTED STRUCTURES
// ============================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    name: String,
    values: Vec<i32>,
    meta: HashMap<String, String>,
    inner: Option<Box<Config>>,
}

#[test]
fn complex_nested_struct_roundtrip() {
    let mut meta = HashMap::new();
    meta.insert("env".to_string(), "prod".to_string());
    let v = Config {
        name: "root".to_string(),
        values: vec![1, 2, 3],
        meta: meta.clone(),
        inner: Some(Box::new(Config {
            name: "child".to_string(),
            values: vec![],
            meta: HashMap::new(),
            inner: None,
        })),
    };
    assert_eq!(roundtrip(&v), v);
}

#[test]
fn hashmap_string_keys_roundtrip() {
    let mut m: HashMap<String, Vec<u64>> = HashMap::new();
    m.insert("a".to_string(), vec![1, 2, 3]);
    m.insert("b".to_string(), vec![]);
    let json = to_string(&m).unwrap();
    let m2: HashMap<String, Vec<u64>> = from_str(&json).unwrap();
    assert_eq!(m, m2);
}

#[test]
fn btreemap_roundtrip_deterministic() {
    let mut m: BTreeMap<String, u32> = BTreeMap::new();
    m.insert("z".to_string(), 1);
    m.insert("a".to_string(), 2);
    m.insert("m".to_string(), 3);
    let json = to_string(&m).unwrap();
    // BTreeMap serializes in key order
    assert_eq!(json, r#"{"a":2,"m":3,"z":1}"#);
    let m2: BTreeMap<String, u32> = from_str(&json).unwrap();
    assert_eq!(m, m2);
}

// ============================================================
// 10. NEWTYPE STRUCTS & TUPLE STRUCTS
// ============================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Wrapper(u64);

#[test]
fn newtype_struct_roundtrip() {
    let v = Wrapper(42);
    let json = to_string(&v).unwrap();
    assert_eq!(json, "42");
    let v2: Wrapper = from_str(&json).unwrap();
    assert_eq!(v, v2);
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Pair(u32, String);

#[test]
fn tuple_struct_roundtrip() {
    let v = Pair(1, "hello".to_string());
    let json = to_string(&v).unwrap();
    assert_eq!(json, r#"[1,"hello"]"#);
    let v2: Pair = from_str(&json).unwrap();
    assert_eq!(v, v2);
}

// ============================================================
// 11. OPTION HANDLING IN STRUCTS
// ============================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct WithOptionalField {
    required: u32,
    optional: Option<String>,
}

#[test]
fn option_field_missing_becomes_none() {
    let v: WithOptionalField = from_str(r#"{"required":1}"#).unwrap();
    assert_eq!(v, WithOptionalField { required: 1, optional: None });
}

#[test]
fn option_field_null_becomes_none() {
    let v: WithOptionalField = from_str(r#"{"required":1,"optional":null}"#).unwrap();
    assert_eq!(v, WithOptionalField { required: 1, optional: None });
}

#[test]
fn option_field_present_becomes_some() {
    let v: WithOptionalField = from_str(r#"{"required":1,"optional":"hello"}"#).unwrap();
    assert_eq!(v, WithOptionalField { required: 1, optional: Some("hello".to_string()) });
}

#[test]
fn option_field_ser_none_outputs_null() {
    let v = WithOptionalField { required: 1, optional: None };
    let json = to_string(&v).unwrap();
    assert!(json.contains("\"optional\":null"), "expected null for None: {json}");
}

// ============================================================
// 12. RENAMED MISSING FIELD (adapted from serde_json test_missing_renamed_field)
// ============================================================

#[test]
fn renamed_field_missing_ok_when_option() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct Foo {
        #[serde(rename = "y")]
        x: Option<u32>,
    }

    let v: Foo = from_str("{}").unwrap();
    assert_eq!(v, Foo { x: None });

    let v: Foo = from_str(r#"{"y":5}"#).unwrap();
    assert_eq!(v, Foo { x: Some(5) });
}

// ============================================================
// 13. FLATTEN
// ============================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FlatBase {
    id: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FlatOuter {
    #[serde(flatten)]
    base: FlatBase,
    extra: String,
}

#[test]
fn flatten_roundtrip() {
    let v = FlatOuter { base: FlatBase { id: 1 }, extra: "hi".to_string() };
    assert_eq!(roundtrip(&v), v);
}

// ============================================================
// 14. UNTAGGED ENUMS
// ============================================================

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum Untagged {
    Int(i64),
    Str(String),
}

#[test]
fn untagged_enum_roundtrip() {
    assert_eq!(roundtrip(&Untagged::Int(42)), Untagged::Int(42));
    assert_eq!(
        roundtrip(&Untagged::Str("hello".to_string())),
        Untagged::Str("hello".to_string())
    );
}
