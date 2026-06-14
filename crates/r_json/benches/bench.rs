use criterion::{black_box, criterion_group, criterion_main, Criterion};
use r_json::{FromJson, ToJson};
use serde::{Deserialize, Serialize};

// ── shared test struct ────────────────────────────────────────────────────────

#[derive(ToJson, FromJson, Debug)]
#[serde(rename_all = "camelCase")]
struct Profile<'a> {
    user_id:    u64,
    name:       &'a str,
    score:      f64,
    active:     bool,
    tag_count:  u32,
}

// serde + serde_json equivalent for comparison
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ProfileSerde {
    user_id:    u64,
    name:       String,
    score:      f64,
    active:     bool,
    tag_count:  u32,
}

const JSON: &str = r#"{"userId":12345,"name":"Alice","score":98.6,"active":true,"tagCount":7}"#;

// ── serialization ─────────────────────────────────────────────────────────────

fn bench_ser_rjson(c: &mut Criterion) {
    let profile = Profile {
        user_id: 12345,
        name: "Alice",
        score: 98.6,
        active: true,
        tag_count: 7,
    };
    c.bench_function("ser/r_json", |b| {
        b.iter(|| black_box(profile.to_json_bytes()))
    });
}

fn bench_ser_serde_json(c: &mut Criterion) {
    let profile = ProfileSerde {
        user_id: 12345,
        name: "Alice".into(),
        score: 98.6,
        active: true,
        tag_count: 7,
    };
    c.bench_function("ser/serde_json", |b| {
        b.iter(|| black_box(serde_json::to_string(&profile).unwrap()))
    });
}

// ── deserialization ───────────────────────────────────────────────────────────

fn bench_de_rjson(c: &mut Criterion) {
    c.bench_function("de/r_json (zero-copy)", |b| {
        b.iter(|| black_box(Profile::from_json_str(JSON).unwrap()))
    });
}

fn bench_de_serde_json(c: &mut Criterion) {
    c.bench_function("de/serde_json", |b| {
        b.iter(|| black_box(serde_json::from_str::<ProfileSerde>(JSON).unwrap()))
    });
}

// ── SIMD / SWAR scanning ──────────────────────────────────────────────────────

fn bench_swar(c: &mut Criterion) {
    let input = b"this is a long string with no special characters at all until the very end\"";
    c.bench_function("scan/swar_u64", |b| {
        b.iter(|| black_box(r_json::simd::find_quote_or_backslash(input, 0)))
    });
    #[cfg(feature = "simd")]
    c.bench_function("scan/swar_u128", |b| {
        b.iter(|| black_box(r_json::simd::find_quote_or_backslash_simd16(input, 0)))
    });
}

criterion_group!(
    benches,
    bench_ser_rjson,
    bench_ser_serde_json,
    bench_de_rjson,
    bench_de_serde_json,
    bench_swar,
);
criterion_main!(benches);
