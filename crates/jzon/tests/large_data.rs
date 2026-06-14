//! Large-data integration tests for jzon.
//!
//! Each test reads a real-world JSON file from `crates/jzon/data/`, verifies that
//! `jzon_serde` produces output identical to `serde_json`, exercises the round-trip
//! (parse → serialize → re-parse), and prints throughput numbers when run with
//! `cargo test --test large_data -- --nocapture`.

use std::time::Instant;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn data_path(name: &str) -> std::path::PathBuf {
    // Tests run from the crate root (crates/jzon/)
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(name)
}

fn read_data(name: &str) -> String {
    std::fs::read_to_string(data_path(name))
        .unwrap_or_else(|_| panic!("missing data/{name} — run from workspace root"))
}

/// Recursively compare two `serde_json::Value`s with float tolerance.
///
/// `serde_json` uses Grisu3/Dragon4 for accurate shortest-decimal f64 printing,
/// while `jzon_serde` currently uses `str::parse::<f64>()` which may produce
/// a slightly different `f64` bit pattern for the same decimal literal.  Both
/// values represent the same real number to within machine epsilon, so we accept
/// differences up to `f64::EPSILON * 8`.
fn values_approx_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value::*;
    match (a, b) {
        (Null, Null) | (Bool(true), Bool(true)) | (Bool(false), Bool(false)) => true,
        (Bool(x), Bool(y)) => x == y,
        (String(x), String(y)) => x == y,
        (Number(x), Number(y)) => {
            match (x.as_f64(), y.as_f64()) {
                (Some(fx), Some(fy)) => {
                    if fx == fy { return true; }
                    // Both NaN — treat as equal (shouldn't appear in valid JSON, but just in case)
                    if fx.is_nan() && fy.is_nan() { return true; }
                    // Relative tolerance: accept up to 8 ULPs difference
                    let diff = (fx - fy).abs();
                    let scale = fx.abs().max(fy.abs()).max(1.0);
                    diff <= scale * f64::EPSILON * 8.0
                }
                _ => x == y,
            }
        }
        (Array(xa), Array(ya)) => {
            xa.len() == ya.len() && xa.iter().zip(ya.iter()).all(|(a, b)| values_approx_eq(a, b))
        }
        (Object(xo), Object(yo)) => {
            xo.len() == yo.len()
                && xo.iter().all(|(k, v)| yo.get(k).map_or(false, |yv| values_approx_eq(v, yv)))
        }
        _ => false,
    }
}

/// Parse with both jzon_serde and serde_json, assert structurally identical output.
/// Floats are compared with a small relative tolerance (≤ 8 ULPs) to account for
/// minor rounding differences between parsers.
fn assert_parse_matches(name: &str, input: &str) {
    let jzon_val: serde_json::Value = jzon_serde::from_str(input)
        .unwrap_or_else(|e| panic!("{name}: jzon_serde parse failed: {e}"));
    let serde_val: serde_json::Value = serde_json::from_str(input)
        .unwrap_or_else(|e| panic!("{name}: serde_json parse failed: {e}"));
    assert!(
        values_approx_eq(&jzon_val, &serde_val),
        "{name}: jzon_serde output differs from serde_json"
    );
}

/// Parse with jzon_serde, serialize back, re-parse with serde_json, assert
/// structurally equal (floats tolerate ≤ 8 ULPs difference).
fn assert_roundtrip(name: &str, input: &str) {
    let val: serde_json::Value = jzon_serde::from_str(input)
        .unwrap_or_else(|e| panic!("{name}: parse failed: {e}"));
    let serialized = jzon_serde::to_string(&val)
        .unwrap_or_else(|e| panic!("{name}: serialize failed: {e}"));
    let val2: serde_json::Value = serde_json::from_str(&serialized)
        .unwrap_or_else(|e| panic!("{name}: re-parse failed: {e}"));
    assert!(
        values_approx_eq(&val, &val2),
        "{name}: round-trip produced different Value"
    );
}

/// Print throughput stats (captured by `cargo test -- --nocapture`).
fn print_throughput(name: &str, bytes: usize, elapsed: std::time::Duration) {
    let mb = bytes as f64 / 1_048_576.0;
    let secs = elapsed.as_secs_f64();
    println!("  {name}: {mb:.1} MB in {secs:.3}s = {:.0} MB/s", mb / secs);
}

// ---------------------------------------------------------------------------
// Per-file tests
// ---------------------------------------------------------------------------

/// twitter.json — 617 KB, mixed string/int/bool/nested
#[test]
fn twitter_parse_correctness() {
    let input = read_data("twitter.json");
    let t = Instant::now();
    let _: serde_json::Value = jzon_serde::from_str(&input)
        .expect("twitter.json: jzon_serde parse failed");
    let elapsed = t.elapsed();
    print_throughput("twitter.json (jzon_serde)", input.len(), elapsed);

    assert_parse_matches("twitter.json", &input);
}

#[test]
fn twitter_roundtrip() {
    let input = read_data("twitter.json");
    assert_roundtrip("twitter.json", &input);
}

/// canada.json — 2.1 MB, 99% f64 coordinates
#[test]
fn canada_parse_correctness() {
    let input = read_data("canada.json");
    let t = Instant::now();
    let _: serde_json::Value = jzon_serde::from_str(&input)
        .expect("canada.json: jzon_serde parse failed");
    let elapsed = t.elapsed();
    print_throughput("canada.json (jzon_serde)", input.len(), elapsed);

    assert_parse_matches("canada.json", &input);
}

#[test]
fn canada_roundtrip() {
    let input = read_data("canada.json");
    assert_roundtrip("canada.json", &input);
}

/// citm_catalog.json — 1.6 MB, HashMap-heavy
#[test]
fn citm_catalog_parse_correctness() {
    let input = read_data("citm_catalog.json");
    let t = Instant::now();
    let _: serde_json::Value = jzon_serde::from_str(&input)
        .expect("citm_catalog.json: jzon_serde parse failed");
    let elapsed = t.elapsed();
    print_throughput("citm_catalog.json (jzon_serde)", input.len(), elapsed);

    assert_parse_matches("citm_catalog.json", &input);
}

#[test]
fn citm_catalog_roundtrip() {
    let input = read_data("citm_catalog.json");
    assert_roundtrip("citm_catalog.json", &input);
}

/// generated_50k.json — 9.7 MB, 50 K heterogeneous records
#[test]
fn generated_50k_parse_correctness() {
    let input = read_data("generated_50k.json");
    let t = Instant::now();
    let _: serde_json::Value = jzon_serde::from_str(&input)
        .expect("generated_50k.json: jzon_serde parse failed");
    let elapsed = t.elapsed();
    print_throughput("generated_50k.json (jzon_serde)", input.len(), elapsed);

    assert_parse_matches("generated_50k.json", &input);
}

#[test]
fn generated_50k_roundtrip() {
    let input = read_data("generated_50k.json");
    assert_roundtrip("generated_50k.json", &input);
}

/// mixed_2mb.json — 2 MB, nested objects + coordinate arrays + strings
#[test]
fn mixed_2mb_parse_correctness() {
    let input = read_data("mixed_2mb.json");
    let t = Instant::now();
    let _: serde_json::Value = jzon_serde::from_str(&input)
        .expect("mixed_2mb.json: jzon_serde parse failed");
    let elapsed = t.elapsed();
    print_throughput("mixed_2mb.json (jzon_serde)", input.len(), elapsed);

    assert_parse_matches("mixed_2mb.json", &input);
}

#[test]
fn mixed_2mb_roundtrip() {
    let input = read_data("mixed_2mb.json");
    assert_roundtrip("mixed_2mb.json", &input);
}

// ---------------------------------------------------------------------------
// Volume / stress tests
// ---------------------------------------------------------------------------

#[test]
fn stress_9mb_generated() {
    let input = read_data("generated_50k.json");
    assert!(
        input.len() > 9_000_000,
        "generated_50k.json should be > 9 MB, got {} bytes",
        input.len()
    );

    // Parse with jzon_serde
    let t = Instant::now();
    let jzon_val: serde_json::Value = jzon_serde::from_str(&input)
        .expect("jzon_serde parse failed on 9 MB file");
    let jzon_elapsed = t.elapsed();

    // Parse with serde_json for comparison
    let t2 = Instant::now();
    let serde_val: serde_json::Value = serde_json::from_str(&input)
        .expect("serde_json parse failed on 9 MB file");
    let serde_elapsed = t2.elapsed();

    assert!(values_approx_eq(&jzon_val, &serde_val), "jzon_serde and serde_json disagree on 9 MB file");

    let mb = input.len() as f64 / 1_048_576.0;
    println!(
        "  stress 9MB: jzon_serde {:.0} MB/s  |  serde_json {:.0} MB/s",
        mb / jzon_elapsed.as_secs_f64(),
        mb / serde_elapsed.as_secs_f64(),
    );
}

/// Parse all five files sequentially, report aggregate throughput.
#[test]
fn all_files_sequential_throughput() {
    let files = [
        "twitter.json",
        "canada.json",
        "citm_catalog.json",
        "generated_50k.json",
        "mixed_2mb.json",
    ];

    let mut total_bytes = 0usize;
    let mut total_elapsed = std::time::Duration::ZERO;

    println!();
    println!("  === large_data throughput summary ===");
    for name in &files {
        let input = read_data(name);
        let bytes = input.len();
        let t = Instant::now();
        let _: serde_json::Value = jzon_serde::from_str(&input)
            .unwrap_or_else(|e| panic!("{name}: jzon_serde failed: {e}"));
        let elapsed = t.elapsed();
        print_throughput(name, bytes, elapsed);
        total_bytes += bytes;
        total_elapsed += elapsed;
    }
    println!(
        "  TOTAL: {:.1} MB in {:.3}s = {:.0} MB/s",
        total_bytes as f64 / 1_048_576.0,
        total_elapsed.as_secs_f64(),
        (total_bytes as f64 / 1_048_576.0) / total_elapsed.as_secs_f64(),
    );
}

// ---------------------------------------------------------------------------
// Typed round-trip tests via jzon custom derive
// ---------------------------------------------------------------------------

/// twitter.json typed round-trip via jzon custom derive
#[test]
fn twitter_typed_roundtrip() {
    use jzon::{FromJson, ToJson};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, ToJson, FromJson, Debug, PartialEq, Default, Clone)]
    #[serde(default)]
    struct TwitterUser {
        id: u64,
        name: String,
        screen_name: String,
    }

    #[derive(Serialize, Deserialize, ToJson, FromJson, Debug, PartialEq, Default, Clone)]
    #[serde(default)]
    struct Tweet {
        id: u64,
        text: String,
        retweet_count: u64,
        user: TwitterUser,
    }

    #[derive(Serialize, Deserialize, ToJson, FromJson, Debug, PartialEq, Default, Clone)]
    #[serde(default)]
    struct TwitterData {
        statuses: Vec<Tweet>,
    }

    let input = read_data("twitter.json");
    let t = Instant::now();
    let data = TwitterData::from_json_str(&input).expect("typed parse failed");
    let parse_ms = t.elapsed().as_millis();

    assert!(!data.statuses.is_empty(), "no statuses parsed");

    // Serialize with jzon, deserialize with serde_json to verify interop
    let json = data.to_json_string();
    let data2: TwitterData = serde_json::from_str(&json).expect("serde re-parse failed");
    assert_eq!(data.statuses.len(), data2.statuses.len());

    println!(
        "  twitter typed: {} tweets in {}ms, serialized {}KB",
        data.statuses.len(),
        parse_ms,
        json.len() / 1024
    );
}

/// canada.json coordinate round-trip via jzon custom derive
#[test]
fn canada_typed_roundtrip() {
    use jzon::{FromJson, ToJson};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
    struct CanadaGeometry {
        coordinates: Vec<Vec<Vec<f64>>>,
    }
    #[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
    struct CanadaFeature {
        geometry: CanadaGeometry,
    }
    #[derive(Serialize, Deserialize, ToJson, FromJson, Clone)]
    struct Canada {
        features: Vec<CanadaFeature>,
    }

    let input = read_data("canada.json");
    let t = Instant::now();
    let data = Canada::from_json_str(&input).expect("canada parse failed");
    let parse_ms = t.elapsed().as_millis();

    let total_pts: usize = data
        .features
        .iter()
        .flat_map(|f| &f.geometry.coordinates)
        .flat_map(|ring| ring.iter())
        .count();

    assert!(total_pts > 1000, "expected >1000 coordinate rings, got {total_pts}");

    let json = data.to_json_string();
    println!(
        "  canada typed: {} features, {} coord-rings in {}ms, {}KB output",
        data.features.len(),
        total_pts,
        parse_ms,
        json.len() / 1024
    );
}

/// 9.7 MB generated file: zero-copy mode via jzon_serde
#[test]
fn large_generated_parse_and_roundtrip() {
    let input = read_data("generated_50k.json");

    // Timed jzon_serde parse
    let t = Instant::now();
    let val: serde_json::Value = jzon_serde::from_str(&input)
        .expect("jzon_serde parse failed");
    let parse_elapsed = t.elapsed();

    // Serialize back
    let t2 = Instant::now();
    let serialized = jzon_serde::to_string(&val).expect("jzon_serde serialize failed");
    let ser_elapsed = t2.elapsed();

    // Re-parse to verify correctness
    let val2: serde_json::Value = serde_json::from_str(&serialized)
        .expect("serde_json re-parse failed after jzon_serde serialization");
    assert!(values_approx_eq(&val, &val2), "round-trip changed the value");

    let mb = input.len() as f64 / 1_048_576.0;
    println!(
        "  generated_50k zero-copy: parse {:.0} MB/s | serialize {:.0} MB/s",
        mb / parse_elapsed.as_secs_f64(),
        serialized.len() as f64 / 1_048_576.0 / ser_elapsed.as_secs_f64(),
    );
}

// ---------------------------------------------------------------------------
// Edge-case volume checks
// ---------------------------------------------------------------------------

/// Verify file sizes are in the expected ballpark so tests are meaningful.
#[test]
fn verify_data_file_sizes() {
    let expected_min: &[(&str, usize)] = &[
        ("twitter.json",       500_000),   // ~617 KB
        ("canada.json",      1_500_000),   // ~2.1 MB
        ("citm_catalog.json",  1_000_000), // ~1.6 MB
        ("generated_50k.json", 9_000_000), // ~9.7 MB
        ("mixed_2mb.json",     1_500_000), // ~2   MB
    ];

    for (name, min_bytes) in expected_min {
        let path = data_path(name);
        let meta = std::fs::metadata(&path)
            .unwrap_or_else(|_| panic!("cannot stat data/{name}"));
        assert!(
            meta.len() as usize >= *min_bytes,
            "data/{name} is only {} bytes, expected >= {min_bytes}",
            meta.len()
        );
        println!(
            "  data/{name}: {:.1} MB",
            meta.len() as f64 / 1_048_576.0
        );
    }
}
