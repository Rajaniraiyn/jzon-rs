// Comparative JSON benchmark: jzon (all 3 modes) vs serde_json vs sonic_rs vs simd_json
// Run with: cargo bench --bench bench_cmp --features "simd,fast-float"

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use jzon::{FromJson, ToJson};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

#[allow(dead_code)]
#[derive(jzon::FromJson, serde::Deserialize, Debug)]
struct BorrowedUser<'a> {
    id: u64,
    #[serde(borrow)]
    name: &'a str,
    score: f64,
}
#[derive(jzon::FromJson, jzon::ToJson, serde::Deserialize, serde::Serialize, Clone, Debug)]
struct OwnedUser {
    id: u64,
    name: String,
    score: f64,
}
#[allow(dead_code)]
static ZERO_COPY_INPUT: &str = r#"{"id":42,"name":"alice_wonderland_2024","score":9.87}"#;

#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct CanadaProperties {
    name: String,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct CanadaGeometry {
    #[serde(rename = "type")]
    geometry_type: String,
    coordinates: Vec<Vec<Vec<f64>>>,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct CanadaFeature {
    #[serde(rename = "type")]
    feature_type: String,
    properties: CanadaProperties,
    geometry: CanadaGeometry,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct Canada {
    #[serde(rename = "type")]
    canada_type: String,
    features: Vec<CanadaFeature>,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone, Default)]
#[serde(default)]
struct TwitterUser {
    id: u64,
    name: String,
    screen_name: String,
    followers_count: u64,
    friends_count: u64,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone, Default)]
#[serde(default)]
struct Tweet {
    id: u64,
    text: String,
    retweet_count: u64,
    favorite_count: u64,
    user: TwitterUser,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone, Default)]
#[serde(default)]
struct TwitterData {
    statuses: Vec<Tweet>,
}
#[derive(Serialize, Deserialize, jzon::ToJson, jzon::FromJson, Clone, Default)]
#[serde(default)]
struct CitmCatalog {
    #[serde(rename = "areaNames")]
    area_names: HashMap<String, String>,
    #[serde(rename = "audienceSubCategoryNames")]
    audience_sub_category_names: HashMap<String, String>,
    #[serde(rename = "blockNames")]
    block_names: HashMap<String, String>,
    #[serde(rename = "seatCategoryNames")]
    seat_category_names: HashMap<String, String>,
    #[serde(rename = "subTopicNames")]
    sub_topic_names: HashMap<String, String>,
    #[serde(rename = "subjectNames")]
    subject_names: HashMap<String, String>,
    #[serde(rename = "topicNames")]
    topic_names: HashMap<String, String>,
    #[serde(rename = "venueNames")]
    venue_names: HashMap<String, String>,
}
#[derive(serde::Serialize, serde::Deserialize, jzon::ToJson, jzon::FromJson, Clone, Default)]
#[serde(default)]
struct GeneratedRecord {
    id:       u64,
    name:     String,
    score:    f64,
    active:   bool,
    tags:     Vec<String>,
    metadata: GeneratedMeta,
}

#[derive(serde::Serialize, serde::Deserialize, jzon::ToJson, jzon::FromJson, Clone, Default)]
#[serde(default)]
struct GeneratedMeta {
    created: String,
    updated: String,
    version: u64,
}

#[derive(serde::Serialize, serde::Deserialize, jzon::ToJson, jzon::FromJson, Clone)]
struct Coord { x: f64, y: f64 }

#[derive(serde::Serialize, serde::Deserialize, jzon::ToJson, jzon::FromJson, Clone, Default)]
#[serde(default)]
struct MixedData {
    flat_array: Vec<Coord>,
    strings: Vec<String>,
    // ponytail: 'nested' is complex/recursive — skipped; jzon ignores unknown fields by default
}

#[derive(serde::Serialize, serde::Deserialize, jzon::ToJson, Clone)]
#[serde(tag = "type")]
enum ShapeEnum {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Triangle { base: f64, height: f64, area: f64 },
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct Point {
    x: f64,
    y: f64,
    z: f64,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct Record {
    id: u64,
    value: f64,
    label: String,
    active: bool,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct Tiny {
    id: u64,
    ok: bool,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct StringHeavy {
    f0: String,
    f1: String,
    f2: String,
    f3: String,
    f4: String,
    f5: String,
    f6: String,
    f7: String,
    f8: String,
    f9: String,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct Deep {
    a: DeepL2,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct DeepL2 {
    b: DeepL3,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct DeepL3 {
    c: DeepL4,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct DeepL4 {
    d: DeepL5,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct DeepL5 {
    e: DeepL6,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct DeepL6 {
    f: i64,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct Wide {
    f01: u64,
    f02: u64,
    f03: u64,
    f04: u64,
    f05: u64,
    f06: u64,
    f07: u64,
    f08: f64,
    f09: f64,
    f10: f64,
    f11: f64,
    f12: bool,
    f13: bool,
    f14: String,
    f15: String,
    f16: u64,
    f17: u64,
    f18: i64,
    f19: i64,
    f20: u64,
}
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct MixedRecord {
    id: u64,
    name: String,
    score: f64,
    active: bool,
    rank: i64,
}
static TINY_ARRAY: OnceLock<String> = OnceLock::new();
fn tiny_array_json() -> &'static str {
    TINY_ARRAY.get_or_init(|| {
        let mut s = String::with_capacity(32 * 1000 + 2);
        s.push('[');
        for i in 0u64..1000 {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(r#"{{"id":{},"ok":true}}"#, i));
        }
        s.push(']');
        s
    })
}
static STRING_HEAVY_JSON: OnceLock<String> = OnceLock::new();
fn string_heavy_json() -> &'static str {
    STRING_HEAVY_JSON.get_or_init(|| {
        let field_val = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let mut s = String::with_capacity(600 * 1024);
        s.push('[');
        for i in 0usize..100 {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                r#"{{"f0":"{0}","f1":"{0}","f2":"{0}","f3":"{0}","f4":"{0}","f5":"{0}","f6":"{0}","f7":"{0}","f8":"{0}","f9":"{0}"}}"#,
                field_val
            ));
        }
        s.push(']');
        s
    })
}
const DEEP_JSON: &str =
    r#"{"a":{"b":{"c":{"d":{"e":{"f":42}}}}}}"#;
static WIDE_JSON: OnceLock<String> = OnceLock::new();
fn wide_json() -> &'static str {
    WIDE_JSON.get_or_init(|| {
        serde_json::to_string(&Wide {
            f01: 1, f02: 2, f03: 3, f04: 4, f05: 5,
            f06: 6, f07: 7, f08: 8.0, f09: 9.0, f10: 10.0,
            f11: 11.0, f12: true, f13: false, f14: "hello".into(),
            f15: "world".into(), f16: 16, f17: 17, f18: -18, f19: -19, f20: 20,
        }).unwrap()
    })
}
static MIXED_ARRAY_JSON: OnceLock<String> = OnceLock::new();
fn mixed_array_json() -> &'static str {
    MIXED_ARRAY_JSON.get_or_init(|| {
        let mut s = String::with_capacity(12 * 1024);
        s.push('[');
        for i in 0usize..200 {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                r#"{{"id":{},"name":"record-{}","score":{:.2},"active":{},"rank":{}}}"#,
                i,
                i,
                (i as f64) * 0.5,
                i % 2 == 0,
                -(i as i64),
            ));
        }
        s.push(']');
        s
    })
}
fn read_data(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e))
}
fn bench_twitter_deser(c: &mut Criterion) {
    let input = read_data("twitter.json");
    let bytes = input.len() as u64;
    let mut group = c.benchmark_group("deserialize/twitter");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let _: TwitterData = serde_json::from_str(black_box(&input)).unwrap();
        })
    });
    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let _: TwitterData = sonic_rs::from_str(black_box(&input)).unwrap();
        })
    });
    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut buf = input.as_bytes().to_vec();
            let _: TwitterData = simd_json::from_slice(black_box(&mut buf)).unwrap();
        })
    });
    group.bench_function("jzon", |b| {
        b.iter(|| {
            let _: TwitterData = TwitterData::from_json_str(black_box(&input)).unwrap();
        })
    });
    group.bench_function("jzon_serde", |b| {
        b.iter(|| {
            let result: TwitterData = jzon_serde::from_str(black_box(&input)).unwrap();
            black_box(result)
        })
    });
    group.finish();
}
fn bench_twitter_ser(c: &mut Criterion) {
    let input = read_data("twitter.json");
    let val: TwitterData = serde_json::from_str(&input).unwrap();
    let bytes = input.len() as u64;
    let mut group = c.benchmark_group("serialize/twitter");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("serde_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&val)).unwrap())
    });
    group.bench_function("sonic_rs", |b| {
        b.iter(|| sonic_rs::to_string(black_box(&val)).unwrap())
    });
    group.bench_function("jzon", |b| {
        b.iter(|| black_box(&val).to_json_bytes())
    });
    group.bench_function("jzon_serde", |b| {
        b.iter(|| black_box(jzon_serde::to_string(black_box(&val)).unwrap()))
    });
    group.finish();
}
fn bench_canada_deser(c: &mut Criterion) {
    let input = read_data("canada.json");
    let bytes = input.len() as u64;
    let mut group = c.benchmark_group("deserialize/canada");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let _: Canada = serde_json::from_str(black_box(&input)).unwrap();
        })
    });
    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let _: Canada = sonic_rs::from_str(black_box(&input)).unwrap();
        })
    });
    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut buf = input.as_bytes().to_vec();
            let _: Canada = simd_json::from_slice(black_box(&mut buf)).unwrap();
        })
    });
    group.bench_function("jzon", |b| {
        b.iter(|| {
            let _: Canada = Canada::from_json_str(black_box(&input)).unwrap();
        })
    });
    group.bench_function("jzon_serde", |b| {
        b.iter(|| {
            let result: Canada = jzon_serde::from_str(black_box(&input)).unwrap();
            black_box(result)
        })
    });
    group.finish();
}
fn bench_canada_ser(c: &mut Criterion) {
    let input = read_data("canada.json");
    let val: Canada = serde_json::from_str(&input).unwrap();
    let bytes = input.len() as u64;
    let mut group = c.benchmark_group("serialize/canada");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("serde_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&val)).unwrap())
    });
    group.bench_function("sonic_rs", |b| {
        b.iter(|| sonic_rs::to_string(black_box(&val)).unwrap())
    });
    group.bench_function("jzon", |b| {
        b.iter(|| black_box(&val).to_json_bytes())
    });
    group.bench_function("jzon_serde", |b| {
        b.iter(|| black_box(jzon_serde::to_string(black_box(&val)).unwrap()))
    });
    group.finish();
}
fn bench_citm_deser(c: &mut Criterion) {
    let input = read_data("citm_catalog.json");
    let bytes = input.len() as u64;
    let mut group = c.benchmark_group("deserialize/citm_catalog");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let _: CitmCatalog = serde_json::from_str(black_box(&input)).unwrap();
        })
    });
    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let _: CitmCatalog = sonic_rs::from_str(black_box(&input)).unwrap();
        })
    });
    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut buf = input.as_bytes().to_vec();
            let _: CitmCatalog = simd_json::from_slice(black_box(&mut buf)).unwrap();
        })
    });
    group.bench_function("jzon_serde", |b| {
        b.iter(|| {
            let result: CitmCatalog = jzon_serde::from_str(black_box(&input)).unwrap();
            black_box(result)
        })
    });
    group.bench_function("jzon/A", |b| {
        b.iter(|| black_box(CitmCatalog::from_json_str(black_box(&input)).unwrap()))
    });
    group.finish();
}
fn bench_micro_deser(c: &mut Criterion) {
    let point_json = r#"{"x":1.5,"y":2.7,"z":-0.3}"#;
    let record_json = r#"{"id":42,"value":3.14,"label":"hello","active":true}"#;

    let mut group = c.benchmark_group("deserialize/micro");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(point_json.len() as u64));
    group.bench_with_input(BenchmarkId::new("serde_json", "Point"), point_json, |b, s| {
        b.iter(|| serde_json::from_str::<Point>(black_box(s)).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("sonic_rs", "Point"), point_json, |b, s| {
        b.iter(|| sonic_rs::from_str::<Point>(black_box(s)).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("simd_json", "Point"), point_json, |b, s| {
        b.iter(|| {
            let mut buf = s.as_bytes().to_vec();
            simd_json::from_slice::<Point>(black_box(&mut buf)).unwrap()
        })
    });
    group.bench_with_input(BenchmarkId::new("jzon", "Point"), point_json, |b, s| {
        b.iter(|| Point::from_json_str(black_box(s)).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("jzon_serde", "Point"), point_json, |b, s| {
        b.iter(|| black_box(jzon_serde::from_str::<Point>(black_box(s)).unwrap()))
    });
    group.throughput(Throughput::Bytes(record_json.len() as u64));
    group.bench_with_input(BenchmarkId::new("serde_json", "Record"), record_json, |b, s| {
        b.iter(|| serde_json::from_str::<Record>(black_box(s)).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("sonic_rs", "Record"), record_json, |b, s| {
        b.iter(|| sonic_rs::from_str::<Record>(black_box(s)).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("simd_json", "Record"), record_json, |b, s| {
        b.iter(|| {
            let mut buf = s.as_bytes().to_vec();
            simd_json::from_slice::<Record>(black_box(&mut buf)).unwrap()
        })
    });
    group.bench_with_input(BenchmarkId::new("jzon", "Record"), record_json, |b, s| {
        b.iter(|| Record::from_json_str(black_box(s)).unwrap())
    });
    group.bench_with_input(BenchmarkId::new("jzon_serde", "Record"), record_json, |b, s| {
        b.iter(|| black_box(jzon_serde::from_str::<Record>(black_box(s)).unwrap()))
    });
    group.finish();
}
fn bench_micro_ser(c: &mut Criterion) {
    let point = Point { x: 1.5, y: 2.7, z: -0.3 };
    let record = Record { id: 42, value: 3.14, label: "hello".to_string(), active: true };
    let point_json = serde_json::to_string(&point).unwrap();
    let record_json = serde_json::to_string(&record).unwrap();

    let mut group = c.benchmark_group("serialize/micro");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(point_json.len() as u64));
    group.bench_function("serde_json/Point", |b| {
        b.iter(|| serde_json::to_string(black_box(&point)).unwrap())
    });
    group.bench_function("sonic_rs/Point", |b| {
        b.iter(|| sonic_rs::to_string(black_box(&point)).unwrap())
    });
    group.bench_function("jzon/Point", |b| {
        b.iter(|| black_box(&point).to_json_bytes())
    });
    group.bench_function("jzon_serde/Point", |b| {
        b.iter(|| black_box(jzon_serde::to_string(black_box(&point)).unwrap()))
    });
    group.throughput(Throughput::Bytes(record_json.len() as u64));
    group.bench_function("serde_json/Record", |b| {
        b.iter(|| serde_json::to_string(black_box(&record)).unwrap())
    });
    group.bench_function("sonic_rs/Record", |b| {
        b.iter(|| sonic_rs::to_string(black_box(&record)).unwrap())
    });
    group.bench_function("jzon/Record", |b| {
        b.iter(|| black_box(&record).to_json_bytes())
    });
    group.bench_function("jzon_serde/Record", |b| {
        b.iter(|| black_box(jzon_serde::to_string(black_box(&record)).unwrap()))
    });

    {
        let mut buf: Vec<u8> = Vec::with_capacity(128);
        group.throughput(Throughput::Bytes(point_json.len() as u64));
        group.bench_function("jzon/Point/pre-alloc", |b| {
            b.iter(|| {
                buf.clear();
                black_box(&point).json_write(&mut buf);
                black_box(buf.len())
            })
        });
        group.bench_function("serde_json/Point/pre-alloc", |b| {
            b.iter(|| {
                buf.clear();
                serde_json::to_writer(&mut buf, black_box(&point)).unwrap();
                black_box(buf.len())
            })
        });
        group.throughput(Throughput::Bytes(record_json.len() as u64));
        group.bench_function("jzon/Record/pre-alloc", |b| {
            b.iter(|| {
                buf.clear();
                black_box(&record).json_write(&mut buf);
                black_box(buf.len())
            })
        });
        group.bench_function("serde_json/Record/pre-alloc", |b| {
            b.iter(|| {
                buf.clear();
                serde_json::to_writer(&mut buf, black_box(&record)).unwrap();
                black_box(buf.len())
            })
        });
    }
    group.finish();
}
fn bench_tiny_array(c: &mut Criterion) {
    let input = tiny_array_json();
    let bytes = input.len() as u64;
    let mut group = c.benchmark_group("deserialize/tiny_array");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("serde_json", |b| {
        b.iter(|| serde_json::from_str::<Vec<Tiny>>(black_box(input)).unwrap())
    });
    group.bench_function("sonic_rs", |b| {
        b.iter(|| sonic_rs::from_str::<Vec<Tiny>>(black_box(input)).unwrap())
    });
    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut buf = input.as_bytes().to_vec();
            simd_json::from_slice::<Vec<Tiny>>(black_box(&mut buf)).unwrap()
        })
    });
    group.bench_function("jzon", |b| {
        b.iter(|| Vec::<Tiny>::from_json_str(black_box(input)).unwrap())
    });
    group.bench_function("jzon_serde", |b| {
        b.iter(|| {
            let result: Vec<Tiny> = jzon_serde::from_str(black_box(input)).unwrap();
            black_box(result)
        })
    });
    group.finish();
}
fn bench_string_heavy(c: &mut Criterion) {
    let input = string_heavy_json();
    let bytes = input.len() as u64;

    {
        let mut group = c.benchmark_group("deserialize/string_heavy");
        group.sample_size(200);
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(4));
        group.throughput(Throughput::Bytes(bytes));
        group.bench_function("serde_json", |b| {
            b.iter(|| serde_json::from_str::<Vec<StringHeavy>>(black_box(input)).unwrap())
        });
        group.bench_function("sonic_rs", |b| {
            b.iter(|| sonic_rs::from_str::<Vec<StringHeavy>>(black_box(input)).unwrap())
        });
        group.bench_function("simd_json", |b| {
            b.iter(|| {
                let mut buf = input.as_bytes().to_vec();
                simd_json::from_slice::<Vec<StringHeavy>>(black_box(&mut buf)).unwrap()
            })
        });
        group.bench_function("jzon", |b| {
            b.iter(|| Vec::<StringHeavy>::from_json_str(black_box(input)).unwrap())
        });
        group.bench_function("jzon_serde", |b| {
            b.iter(|| {
                let result: Vec<StringHeavy> = jzon_serde::from_str(black_box(input)).unwrap();
                black_box(result)
            })
        });
        group.finish();
    }

    {
        let val: Vec<StringHeavy> = serde_json::from_str(input).unwrap();
        let mut group = c.benchmark_group("serialize/string_heavy");
        group.sample_size(200);
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(4));
        group.throughput(Throughput::Bytes(bytes));
        group.bench_function("serde_json", |b| {
            b.iter(|| serde_json::to_string(black_box(&val)).unwrap())
        });
        group.bench_function("sonic_rs", |b| {
            b.iter(|| sonic_rs::to_string(black_box(&val)).unwrap())
        });
        group.bench_function("jzon", |b| {
            b.iter(|| black_box(&val).to_json_bytes())
        });
        group.bench_function("jzon_serde", |b| {
            b.iter(|| black_box(jzon_serde::to_string(black_box(&val)).unwrap()))
        });
        group.finish();
    }
}
fn bench_deep_nested(c: &mut Criterion) {
    let bytes = DEEP_JSON.len() as u64;
    let mut group = c.benchmark_group("deserialize/deep_nested");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("serde_json", |b| {
        b.iter(|| serde_json::from_str::<Deep>(black_box(DEEP_JSON)).unwrap())
    });
    group.bench_function("sonic_rs", |b| {
        b.iter(|| sonic_rs::from_str::<Deep>(black_box(DEEP_JSON)).unwrap())
    });
    group.bench_function("simd_json", |b| {
        b.iter(|| {
            let mut buf = DEEP_JSON.as_bytes().to_vec();
            simd_json::from_slice::<Deep>(black_box(&mut buf)).unwrap()
        })
    });
    group.bench_function("jzon", |b| {
        b.iter(|| Deep::from_json_str(black_box(DEEP_JSON)).unwrap())
    });
    group.finish();
}
fn bench_wide_struct(c: &mut Criterion) {
    let input = wide_json();
    let bytes = input.len() as u64;

    {
        let mut group = c.benchmark_group("deserialize/wide_struct");
        group.sample_size(200);
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(4));
        group.throughput(Throughput::Bytes(bytes));
        group.bench_function("serde_json", |b| {
            b.iter(|| serde_json::from_str::<Wide>(black_box(input)).unwrap())
        });
        group.bench_function("sonic_rs", |b| {
            b.iter(|| sonic_rs::from_str::<Wide>(black_box(input)).unwrap())
        });
        group.bench_function("simd_json", |b| {
            b.iter(|| {
                let mut buf = input.as_bytes().to_vec();
                simd_json::from_slice::<Wide>(black_box(&mut buf)).unwrap()
            })
        });
        group.bench_function("jzon", |b| {
            b.iter(|| Wide::from_json_str(black_box(input)).unwrap())
        });
        group.finish();
    }

    {
        let val: Wide = serde_json::from_str(input).unwrap();
        let mut group = c.benchmark_group("serialize/wide_struct");
        group.sample_size(200);
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(4));
        group.throughput(Throughput::Bytes(bytes));
        group.bench_function("serde_json", |b| {
            b.iter(|| serde_json::to_string(black_box(&val)).unwrap())
        });
        group.bench_function("sonic_rs", |b| {
            b.iter(|| sonic_rs::to_string(black_box(&val)).unwrap())
        });
        group.bench_function("jzon", |b| {
            b.iter(|| black_box(&val).to_json_bytes())
        });
        group.finish();
    }
}
fn bench_mixed_array(c: &mut Criterion) {
    let input = mixed_array_json();
    let bytes = input.len() as u64;

    {
        let mut group = c.benchmark_group("deserialize/mixed_array");
        group.sample_size(200);
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(4));
        group.throughput(Throughput::Bytes(bytes));
        group.bench_function("serde_json", |b| {
            b.iter(|| serde_json::from_str::<Vec<MixedRecord>>(black_box(input)).unwrap())
        });
        group.bench_function("sonic_rs", |b| {
            b.iter(|| sonic_rs::from_str::<Vec<MixedRecord>>(black_box(input)).unwrap())
        });
        group.bench_function("simd_json", |b| {
            b.iter(|| {
                let mut buf = input.as_bytes().to_vec();
                simd_json::from_slice::<Vec<MixedRecord>>(black_box(&mut buf)).unwrap()
            })
        });
        group.bench_function("jzon", |b| {
            b.iter(|| Vec::<MixedRecord>::from_json_str(black_box(input)).unwrap())
        });
        group.bench_function("jzon_serde", |b| {
            b.iter(|| {
                let result: Vec<MixedRecord> = jzon_serde::from_str(black_box(input)).unwrap();
                black_box(result)
            })
        });
        group.finish();
    }

    {
        let val: Vec<MixedRecord> = serde_json::from_str(input).unwrap();
        let mut group = c.benchmark_group("serialize/mixed_array");
        group.sample_size(200);
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(4));
        group.throughput(Throughput::Bytes(bytes));
        group.bench_function("serde_json", |b| {
            b.iter(|| serde_json::to_string(black_box(&val)).unwrap())
        });
        group.bench_function("sonic_rs", |b| {
            b.iter(|| sonic_rs::to_string(black_box(&val)).unwrap())
        });
        group.bench_function("jzon", |b| {
            b.iter(|| black_box(&val).to_json_bytes())
        });
        group.bench_function("jzon_serde", |b| {
            b.iter(|| black_box(jzon_serde::to_string(black_box(&val)).unwrap()))
        });
        group.finish();
    }
}
fn bench_pre_alloc(c: &mut Criterion) {
    let input = read_data("twitter.json");
    let twittejzon_len = input.len();
    let twitter_val: TwitterData = serde_json::from_str(&input).unwrap();
    let serde_twitter_val: TwitterData = serde_json::from_str(&input).unwrap();

    let mut g = c.benchmark_group("serialize/pre_alloc");
    g.throughput(Throughput::Bytes(twittejzon_len as u64));
    g.sample_size(200);
    g.warm_up_time(Duration::from_secs(1));
    g.measurement_time(Duration::from_secs(4));

    let mut buf: Vec<u8> = Vec::with_capacity(twittejzon_len);
    g.bench_function("jzon/reuse", |b| {
        b.iter(|| {
            buf.clear();
            twitter_val.json_write(&mut buf);
            black_box(buf.len())
        })
    });
    g.bench_function("serde_json/reuse", |b| {
        b.iter(|| {
            buf.clear();
            serde_json::to_writer(&mut buf, black_box(&serde_twitter_val)).unwrap();
            black_box(buf.len())
        })
    });
    g.bench_function("jzon_serde/reuse", |b| {
        b.iter(|| {
            buf.clear();
            jzon_serde::to_writer(&mut buf, black_box(&serde_twitter_val)).unwrap();
            black_box(buf.len())
        })
    });
    g.bench_function("sonic_rs/reuse", |b| {
        b.iter(|| {
            let s = sonic_rs::to_string(black_box(&serde_twitter_val)).unwrap();
            black_box(s)
        })
    });
    g.finish();
}
fn bench_zero_copy(c: &mut Criterion) {
    static INPUT: &str = r#"{"id":42,"name":"alice_wonderland_2024","score":9.87}"#;
    let bytes = INPUT.len() as u64;

    let mut g = c.benchmark_group("deserialize/zero_copy");
    g.throughput(criterion::Throughput::Bytes(bytes));
    g.sample_size(500);
    g.warm_up_time(std::time::Duration::from_secs(1));
    g.measurement_time(std::time::Duration::from_secs(3));
    g.bench_function("jzon/A/borrowed", |b| {
        b.iter(|| criterion::black_box(BorrowedUser::from_json_str(INPUT).unwrap()))
    });
    g.bench_function("jzon/A/owned", |b| {
        b.iter(|| criterion::black_box(OwnedUser::from_json_str(INPUT).unwrap()))
    });
    g.bench_function("jzon/B/borrowed", |b| {
        b.iter(|| criterion::black_box(jzon_serde::from_str::<BorrowedUser>(INPUT).unwrap()))
    });
    g.bench_function("jzon/B/owned", |b| {
        b.iter(|| criterion::black_box(jzon_serde::from_str::<OwnedUser>(INPUT).unwrap()))
    });
    g.bench_function("serde_json/borrowed", |b| {
        b.iter(|| criterion::black_box(serde_json::from_str::<BorrowedUser>(INPUT).unwrap()))
    });
    g.bench_function("serde_json/owned", |b| {
        b.iter(|| criterion::black_box(serde_json::from_str::<OwnedUser>(INPUT).unwrap()))
    });
    g.bench_function("sonic_rs", |b| {
        b.iter(|| criterion::black_box(sonic_rs::from_str::<OwnedUser>(INPUT).unwrap()))
    });
    g.finish();
}
fn bench_fixed_buf(c: &mut Criterion) {
    use jzon::ToJsonExt;

    let p = Point { x: 1.5, y: -2.0, z: 3.14 };
    let expected_len = p.to_json_bytes().len();

    let mut g = c.benchmark_group("serialize/fixed_buf");
    g.throughput(criterion::Throughput::Bytes(expected_len as u64));
    g.sample_size(500);
    g.warm_up_time(std::time::Duration::from_secs(1));
    g.measurement_time(std::time::Duration::from_secs(3));
    g.bench_function("jzon/A/Vec_alloc", |b| {
        b.iter(|| criterion::black_box(p.to_json_bytes()))
    });
    g.bench_function("jzon/A/FixedBuf<128>", |b| {
        b.iter(|| criterion::black_box(p.to_fixed_buf::<128>()))
    });
    let mut buf = Vec::with_capacity(128);
    g.bench_function("jzon/A/reuse", |b| {
        b.iter(|| {
            buf.clear();
            p.json_write(&mut buf);
            criterion::black_box(buf.len())
        })
    });
    g.bench_function("serde_json/alloc", |b| {
        b.iter(|| criterion::black_box(serde_json::to_string(&p).unwrap()))
    });
    let mut sj_buf: Vec<u8> = Vec::with_capacity(128);
    g.bench_function("serde_json/reuse", |b| {
        b.iter(|| {
            sj_buf.clear();
            serde_json::to_writer(&mut sj_buf, &p).unwrap();
            criterion::black_box(sj_buf.len())
        })
    });
    g.bench_function("sonic_rs", |b| {
        b.iter(|| criterion::black_box(sonic_rs::to_string(&p).unwrap()))
    });
    g.finish();
}
fn bench_hashmap(c: &mut Criterion) {
    static HASHMAP_JSON: OnceLock<String> = OnceLock::new();
    let input = HASHMAP_JSON.get_or_init(|| {
        let mut s = String::from("{");
        for i in 0u32..200 {
            if i > 0 { s.push(','); }
            s.push_str(&format!(r#""key_{i:04}":"value_lorem_ipsum_{i:06}""#));
        }
        s.push('}');
        s
    });
    let bytes = input.len() as u64;

    let mut g = c.benchmark_group("deserialize/hashmap");
    g.throughput(Throughput::Bytes(bytes));
    g.sample_size(200);
    g.warm_up_time(Duration::from_secs(1));
    g.measurement_time(Duration::from_secs(4));
    g.bench_function("jzon/A", |b| {
        b.iter(|| {
            let m: std::collections::HashMap<String, String> =
                jzon::FromJson::from_json_str(black_box(input)).unwrap();
            black_box(m)
        })
    });
    g.bench_function("jzon/B", |b| {
        b.iter(|| black_box(jzon_serde::from_str::<std::collections::HashMap<String, String>>(black_box(input)).unwrap()))
    });
    g.bench_function("serde_json", |b| {
        b.iter(|| black_box(serde_json::from_str::<std::collections::HashMap<String, String>>(black_box(input)).unwrap()))
    });
    g.bench_function("sonic_rs", |b| {
        b.iter(|| black_box(sonic_rs::from_str::<std::collections::HashMap<String, String>>(black_box(input)).unwrap()))
    });
    g.finish();

    let map: std::collections::HashMap<String, String> = serde_json::from_str(input).unwrap();
    let mut g2 = c.benchmark_group("serialize/hashmap");
    g2.throughput(Throughput::Bytes(bytes));
    g2.sample_size(200);
    g2.warm_up_time(Duration::from_secs(1));
    g2.measurement_time(Duration::from_secs(4));
    g2.bench_function("jzon/A", |b| b.iter(|| black_box(map.to_json_bytes())));
    g2.bench_function("jzon/B", |b| b.iter(|| black_box(jzon_serde::to_string(&map).unwrap())));
    g2.bench_function("serde_json", |b| b.iter(|| black_box(serde_json::to_string(&map).unwrap())));
    g2.bench_function("sonic_rs", |b| b.iter(|| black_box(sonic_rs::to_string(&map).unwrap())));
    g2.finish();
}
fn bench_enum_variants(c: &mut Criterion) {
    static ENUM_JSON: OnceLock<String> = OnceLock::new();
    let input = ENUM_JSON.get_or_init(|| {
        let mut s = String::from("[");
        for i in 0u32..1000 {
            if i > 0 { s.push(','); }
            match i % 3 {
                0 => s.push_str(&format!(r#"{{"type":"Circle","radius":{:.4}}}"#, (i as f64) * 0.1)),
                1 => s.push_str(&format!(r#"{{"type":"Rectangle","width":{:.3},"height":{:.3}}}"#, i as f64, (i + 1) as f64)),
                _ => s.push_str(&format!(r#"{{"type":"Triangle","base":{:.3},"height":{:.3},"area":{:.6}}}"#, i as f64, (i + 1) as f64, (i as f64) * (i + 1) as f64 * 0.5)),
            }
        }
        s.push(']');
        s
    });
    let bytes = input.len() as u64;

    let mut g = c.benchmark_group("deserialize/enum_variants");
    g.throughput(Throughput::Bytes(bytes));
    g.sample_size(100);
    g.warm_up_time(Duration::from_secs(1));
    g.measurement_time(Duration::from_secs(4));
    g.bench_function("jzon/B", |b| {
        b.iter(|| black_box(jzon_serde::from_str::<Vec<ShapeEnum>>(black_box(input)).unwrap()))
    });
    g.bench_function("serde_json", |b| {
        b.iter(|| black_box(serde_json::from_str::<Vec<ShapeEnum>>(black_box(input)).unwrap()))
    });
    g.bench_function("sonic_rs", |b| {
        b.iter(|| black_box(sonic_rs::from_str::<Vec<ShapeEnum>>(black_box(input)).unwrap()))
    });
    g.finish();

    let shapes: Vec<ShapeEnum> = serde_json::from_str(input).unwrap();
    let mut g2 = c.benchmark_group("serialize/enum_variants");
    g2.throughput(Throughput::Bytes(bytes));
    g2.sample_size(100);
    g2.warm_up_time(Duration::from_secs(1));
    g2.measurement_time(Duration::from_secs(4));
    g2.bench_function("jzon/A", |b| b.iter(|| black_box(shapes.to_json_bytes())));
    g2.bench_function("jzon/B", |b| b.iter(|| black_box(jzon_serde::to_string(&shapes).unwrap())));
    g2.bench_function("serde_json", |b| b.iter(|| black_box(serde_json::to_string(&shapes).unwrap())));
    g2.bench_function("sonic_rs", |b| b.iter(|| black_box(sonic_rs::to_string(&shapes).unwrap())));
    g2.finish();
}

fn bench_generated_50k(c: &mut Criterion) {
    let input = read_data("generated_50k.json");
    let bytes = input.len() as u64;

    {
        let mut g = c.benchmark_group("deserialize/generated_50k");
        g.throughput(Throughput::Bytes(bytes));
        g.sample_size(15);
        g.warm_up_time(Duration::from_millis(500));
        g.measurement_time(Duration::from_secs(4));

        g.bench_function("serde_json", |b| {
            b.iter(|| serde_json::from_str::<serde_json::Value>(black_box(&input)).unwrap())
        });
        g.bench_function("sonic_rs", |b| {
            b.iter(|| sonic_rs::from_str::<serde_json::Value>(black_box(&input)).unwrap())
        });
        g.bench_function("jzon/A", |b| {
            b.iter(|| Vec::<GeneratedRecord>::from_json_str(black_box(&input)).unwrap())
        });
        g.bench_function("jzon/B", |b| {
            b.iter(|| jzon_serde::from_str::<serde_json::Value>(black_box(&input)).unwrap())
        });
        g.finish();
    }

    {
        let val: Vec<GeneratedRecord> = serde_json::from_str(&input).unwrap();
        let mut g = c.benchmark_group("serialize/generated_50k");
        g.throughput(Throughput::Bytes(bytes));
        g.sample_size(15);
        g.warm_up_time(Duration::from_millis(500));
        g.measurement_time(Duration::from_secs(4));

        g.bench_function("serde_json", |b| b.iter(|| serde_json::to_string(black_box(&val)).unwrap()));
        g.bench_function("sonic_rs",   |b| b.iter(|| sonic_rs::to_string(black_box(&val)).unwrap()));
        g.bench_function("jzon/A",     |b| b.iter(|| black_box(&val).to_json_bytes()));
        g.bench_function("jzon/B",     |b| b.iter(|| jzon_serde::to_string(black_box(&val)).unwrap()));
        g.finish();
    }
}

fn bench_mixed_2mb(c: &mut Criterion) {
    let input = read_data("mixed_2mb.json");
    let bytes = input.len() as u64;

    {
        let mut g = c.benchmark_group("deserialize/mixed_2mb");
        g.throughput(Throughput::Bytes(bytes));
        g.sample_size(30);
        g.warm_up_time(Duration::from_secs(1));
        g.measurement_time(Duration::from_secs(5));

        g.bench_function("serde_json", |b| {
            b.iter(|| serde_json::from_str::<serde_json::Value>(black_box(&input)).unwrap())
        });
        g.bench_function("sonic_rs", |b| {
            b.iter(|| sonic_rs::from_str::<serde_json::Value>(black_box(&input)).unwrap())
        });
        g.bench_function("jzon/B", |b| {
            b.iter(|| jzon_serde::from_str::<serde_json::Value>(black_box(&input)).unwrap())
        });
        g.bench_function("jzon/A", |b| {
            b.iter(|| MixedData::from_json_str(black_box(&input)).unwrap())
        });
        g.finish();
    }

    {
        let typed_val: MixedData = serde_json::from_str(&input).unwrap();
        let val: serde_json::Value = serde_json::from_str(&input).unwrap();
        let mut g = c.benchmark_group("serialize/mixed_2mb");
        g.throughput(Throughput::Bytes(bytes));
        g.sample_size(30);
        g.warm_up_time(Duration::from_secs(1));
        g.measurement_time(Duration::from_secs(5));

        g.bench_function("serde_json", |b| b.iter(|| serde_json::to_string(black_box(&val)).unwrap()));
        g.bench_function("sonic_rs",   |b| b.iter(|| sonic_rs::to_string(black_box(&val)).unwrap()));
        g.bench_function("jzon/B",     |b| b.iter(|| jzon_serde::to_string(black_box(&val)).unwrap()));
        g.bench_function("jzon/A",     |b| b.iter(|| black_box(&typed_val).to_json_bytes()));
        g.finish();
    }
}

criterion_group!(
    benches,
    bench_twitter_deser,
    bench_twitter_ser,
    bench_canada_deser,
    bench_canada_ser,
    bench_citm_deser,
    bench_micro_deser,
    bench_micro_ser,
    bench_tiny_array,
    bench_string_heavy,
    bench_deep_nested,
    bench_wide_struct,
    bench_mixed_array,
    bench_pre_alloc,
    bench_zero_copy,
    bench_fixed_buf,
    bench_hashmap,
    bench_enum_variants,
    bench_generated_50k,
    bench_mixed_2mb,
);
criterion_main!(benches);
