// Comparative JSON benchmark: r_json vs serde_json vs sonic_rs vs simd_json
// Run with: cargo bench --bench bench_cmp

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use r_json::{FromJson, ToJson};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

// ── Canada structs ────────────────────────────────────────────────────────────

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

// ── Twitter structs ───────────────────────────────────────────────────────────

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

// ── citm_catalog structs (serde-only — uses HashMap, not supported by r_json) ─

// citm_catalog top-level name maps use HashMap<String, String>.
// r_json does not implement FromJson for HashMap, so we benchmark only
// serde_json / sonic_rs / simd_json on this dataset.
#[derive(Serialize, Deserialize, Clone)]
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

impl Default for CitmCatalog {
    fn default() -> Self {
        Self {
            area_names: Default::default(),
            audience_sub_category_names: Default::default(),
            block_names: Default::default(),
            seat_category_names: Default::default(),
            sub_topic_names: Default::default(),
            subject_names: Default::default(),
            topic_names: Default::default(),
            venue_names: Default::default(),
        }
    }
}

// ── Micro-benchmark structs ───────────────────────────────────────────────────

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

// ── New benchmark structs ─────────────────────────────────────────────────────

/// Tiny object for hot-path micro benchmark.
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct Tiny {
    id: u64,
    ok: bool,
}

/// Large string-heavy object — 10 string fields ~50 chars each.
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

/// Deeply nested object (5 levels).
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

/// Wide struct with 20 fields — exercises PHF dispatch.
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

/// Mixed record for array benchmark.
#[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
struct MixedRecord {
    id: u64,
    name: String,
    score: f64,
    active: bool,
    rank: i64,
}

// ── Static test data ──────────────────────────────────────────────────────────

/// 1000-element array of tiny objects: [{"id":0,"ok":true}, ...]
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

/// 500 KB JSON: 100 objects each with 10 string fields (~50 chars each).
static STRING_HEAVY_JSON: OnceLock<String> = OnceLock::new();

fn string_heavy_json() -> &'static str {
    STRING_HEAVY_JSON.get_or_init(|| {
        // Each string field is ~50 ASCII chars.
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

/// Deeply nested JSON.
const DEEP_JSON: &str =
    r#"{"a":{"b":{"c":{"d":{"e":{"f":42}}}}}}"#;

/// Wide struct JSON (20 fields).
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

/// Mixed array: 200 MixedRecord objects (~10 KB).
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

// ── helpers ───────────────────────────────────────────────────────────────────

fn read_data(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e))
}

// ── Twitter benchmarks ────────────────────────────────────────────────────────

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

    group.bench_function("r_json", |b| {
        b.iter(|| {
            let _: TwitterData = TwitterData::from_json_str(black_box(&input)).unwrap();
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

    group.bench_function("r_json", |b| {
        b.iter(|| black_box(&val).to_json_bytes())
    });

    group.finish();
}

// ── Canada benchmarks ─────────────────────────────────────────────────────────

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

    group.bench_function("r_json", |b| {
        b.iter(|| {
            let _: Canada = Canada::from_json_str(black_box(&input)).unwrap();
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

    group.bench_function("r_json", |b| {
        b.iter(|| black_box(&val).to_json_bytes())
    });

    group.finish();
}

// ── citm_catalog benchmarks (serde-only) ─────────────────────────────────────

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

    group.finish();
}

// ── Micro benchmarks ──────────────────────────────────────────────────────────

fn bench_micro_deser(c: &mut Criterion) {
    let point_json = r#"{"x":1.5,"y":2.7,"z":-0.3}"#;
    let record_json = r#"{"id":42,"value":3.14,"label":"hello","active":true}"#;

    let mut group = c.benchmark_group("deserialize/micro");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));

    // Point
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
    group.bench_with_input(BenchmarkId::new("r_json", "Point"), point_json, |b, s| {
        b.iter(|| Point::from_json_str(black_box(s)).unwrap())
    });

    // Record
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
    group.bench_with_input(BenchmarkId::new("r_json", "Record"), record_json, |b, s| {
        b.iter(|| Record::from_json_str(black_box(s)).unwrap())
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

    // Point
    group.throughput(Throughput::Bytes(point_json.len() as u64));
    group.bench_function("serde_json/Point", |b| {
        b.iter(|| serde_json::to_string(black_box(&point)).unwrap())
    });
    group.bench_function("sonic_rs/Point", |b| {
        b.iter(|| sonic_rs::to_string(black_box(&point)).unwrap())
    });
    group.bench_function("r_json/Point", |b| {
        b.iter(|| black_box(&point).to_json_bytes())
    });

    // Record
    group.throughput(Throughput::Bytes(record_json.len() as u64));
    group.bench_function("serde_json/Record", |b| {
        b.iter(|| serde_json::to_string(black_box(&record)).unwrap())
    });
    group.bench_function("sonic_rs/Record", |b| {
        b.iter(|| sonic_rs::to_string(black_box(&record)).unwrap())
    });
    group.bench_function("r_json/Record", |b| {
        b.iter(|| black_box(&record).to_json_bytes())
    });

    // ── Pre-allocated "fair" variants: both write to a reused Vec to remove
    // allocation cost from the comparison.  This isolates serialization logic
    // speed from allocator overhead.
    {
        let mut buf: Vec<u8> = Vec::with_capacity(128);

        group.throughput(Throughput::Bytes(point_json.len() as u64));
        group.bench_function("r_json/Point/pre-alloc", |b| {
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
        group.bench_function("r_json/Record/pre-alloc", |b| {
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

// ── New benchmark: tiny object array (1000 elements) ─────────────────────────

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

    group.bench_function("r_json", |b| {
        b.iter(|| Vec::<Tiny>::from_json_str(black_box(input)).unwrap())
    });

    group.finish();
}

// ── New benchmark: large string-heavy payload (ser + de) ─────────────────────

fn bench_string_heavy(c: &mut Criterion) {
    let input = string_heavy_json();
    let bytes = input.len() as u64;

    // deserialization
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

        group.bench_function("r_json", |b| {
            b.iter(|| Vec::<StringHeavy>::from_json_str(black_box(input)).unwrap())
        });

        group.finish();
    }

    // serialization
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

        group.bench_function("r_json", |b| {
            b.iter(|| black_box(&val).to_json_bytes())
        });

        group.finish();
    }
}

// ── New benchmark: deeply nested object ──────────────────────────────────────

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

    group.bench_function("r_json", |b| {
        b.iter(|| Deep::from_json_str(black_box(DEEP_JSON)).unwrap())
    });

    group.finish();
}

// ── New benchmark: wide struct (20 fields, PHF dispatch) ─────────────────────

fn bench_wide_struct(c: &mut Criterion) {
    let input = wide_json();
    let bytes = input.len() as u64;

    // deserialization
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

        group.bench_function("r_json", |b| {
            b.iter(|| Wide::from_json_str(black_box(input)).unwrap())
        });

        group.finish();
    }

    // serialization
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

        group.bench_function("r_json", |b| {
            b.iter(|| black_box(&val).to_json_bytes())
        });

        group.finish();
    }
}

// ── New benchmark: mixed array (200 MixedRecord objects) ─────────────────────

fn bench_mixed_array(c: &mut Criterion) {
    let input = mixed_array_json();
    let bytes = input.len() as u64;

    // deserialization
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

        group.bench_function("r_json", |b| {
            b.iter(|| Vec::<MixedRecord>::from_json_str(black_box(input)).unwrap())
        });

        group.finish();
    }

    // serialization
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

        group.bench_function("r_json", |b| {
            b.iter(|| black_box(&val).to_json_bytes())
        });

        group.finish();
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
);
criterion_main!(benches);
