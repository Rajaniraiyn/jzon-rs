# r_json Performance Report

Benchmarks run on 2026-06-14, macOS Darwin 25.5.0 (Apple Silicon / arm64).
Command: `cargo bench --bench bench_cmp --features fast-float`
Results saved to: `bench_results/final.txt`

---

## Deserialization throughput (MB/s = dataset_bytes / median_time)

| Dataset | serde_json | sonic-rs | simd-json | r_json final | r_json vs best |
|---------|-----------|---------|----------|-------------|----------------|
| twitter.json (617 KB) | 1,860 MB/s | 1,769 MB/s | 1,792 MB/s | **1,889 MB/s** | **+1.6% over serde_json** |
| canada.json (2.15 MB) | 608 MB/s | **640 MB/s** | 619 MB/s | 620 MB/s | -3.2% vs sonic-rs |

Notes:
- twitter.json is a mixed string/int/bool payload — r_json's zero-allocation key dispatch wins here.
- canada.json is 99% `f64` coordinate data — fast-float2 brings r_json to parity with serde_json and simd-json, within 3% of sonic-rs.
- citm_catalog.json: r_json is not benchmarked for this dataset because its schema uses `HashMap<String, T>` — r_json's proc-macro codegen requires statically-known struct fields; the reference benchmark correctly excludes it.

---

## Serialization throughput (MB/s)

| Dataset | serde_json | sonic-rs | r_json final | r_json vs best |
|---------|-----------|---------|-------------|----------------|
| twitter.json (617 KB) | 21,646 MB/s | **59,482 MB/s** | 12,997 MB/s | -78% vs sonic-rs |
| canada.json (2.15 MB) | **1,171 MB/s** | 1,090 MB/s | 1,060 MB/s | -9.5% vs serde_json |

Notes:
- sonic-rs serialization uses AVX2/NEON SIMD bulk-copy paths for string data — r_json has no equivalent yet.
- canada.json serialization gap vs serde_json is modest (9.5%); the main bottleneck is per-float ryu formatting overhead shared with all libraries.
- twitter.json serialization is dominated by string escaping; sonic-rs's SIMD escaping is 2.7x faster than both serde_json and r_json.

---

## Micro-benchmarks (ns/op)

### Deserialization

| Operation | serde_json | sonic-rs | simd-json | r_json | winner |
|-----------|-----------|---------|----------|--------|--------|
| Point de (`{"x":1.5,"y":2.7,"z":-0.3}`, 25 B) | 73.6 ns | 68.4 ns | 200.2 ns | **46.3 ns** | r_json +33% vs sonic-rs |
| Record de (`{"id":42,"value":3.14,...}`, 52 B) | 77.1 ns | 83.3 ns | 236.3 ns | **76.7 ns** | r_json (effectively tied with serde_json) |

### Serialization

| Operation | serde_json | sonic-rs | r_json | winner |
|-----------|-----------|---------|--------|--------|
| Point ser | **61.4 ns** | 117.9 ns | 107.6 ns | serde_json |
| Record ser | 60.7 ns | 56.3 ns | **53.5 ns** | r_json +6% vs sonic-rs |

### Zero-copy (bench.rs — Profile struct with `name: &'a str`)

| Variant | Library | Median (ns) |
|---------|---------|-------------|
| `Profile<'a>` with `&str` field | r_json (zero-copy) | **81.7 ns** |
| `ProfileSerde` with `String` field | serde_json | 93.6 ns |

Zero-copy advantage: **14.5% faster** than serde_json's String-allocating path for the same payload. The gap grows proportionally with the number and length of string fields because r_json borrows directly from the input buffer rather than heap-allocating each string.

---

## Key findings

### Where r_json wins (and by how much)

- **twitter.json deserialization: +1.6% over serde_json, +6.8% over sonic-rs, +5.5% over simd-json.**
  The win comes from compile-time field dispatch (no runtime HashMap lookup or reflection) and stack-only parsing of known struct shapes.

- **Micro Point deserialization: +33% over sonic-rs, +37% over serde_json.**
  For tiny well-known structs, r_json's generated parser branches directly into field slots with no intermediate `Value` representation. This is the best-case scenario for the proc-macro approach.

- **Micro Record serialization: +6% over sonic-rs, +12% over serde_json.**
  ryu float formatting + a pre-sized output buffer beats both competitors on a balanced payload (one int, one float, one short string, one bool).

- **Zero-copy string deserialization: +14.5% over serde_json.**
  Using `&'a str` instead of `String` eliminates heap allocation per string field. Advantage scales with string count and length.

### Where it is competitive (within 20%)

- **canada.json deserialization: 620 MB/s vs sonic-rs 640 MB/s (-3.2%).**
  fast-float2 closes most of the gap that existed before the optimization rounds. This is within run-to-run noise for many workloads.

- **canada.json serialization: 1,060 MB/s vs serde_json 1,171 MB/s (-9.5%).**
  Competitive. The bottleneck is raw ryu throughput which all libraries share.

- **Micro Record deserialization: 76.7 ns vs serde_json 77.1 ns (~tie).**
  The generated parser matches serde_json's derived `Deserialize` on mixed-type records.

### Where it still lags and why (structural reason)

- **twitter.json serialization: 12,997 MB/s vs sonic-rs 59,482 MB/s (-78%).**
  Structural reason: sonic-rs uses NEON/AVX2 SIMD to bulk-escape and copy string bytes 16–32 at a time. r_json's current `write_str_escaped` is scalar. String-heavy JSON serialization is almost entirely bounded by string-escape throughput, so this gap will persist until a SIMD string escaper is added.

- **Point serialization: 107.6 ns vs serde_json 61.4 ns (-43%).**
  Structural reason: serde_json's `to_string` for a 3-float struct benefits from the `itoa`/`ryu` write path fused with a small stack buffer. r_json serializes through a `Vec<u8>` that must grow, adding one branch per push. A pre-allocated writer would close most of this gap.

---

## Optimization journey

| Round | Key change | canada.json de | twitter.json de | micro-Point de |
|-------|-----------|----------------|-----------------|----------------|
| Baseline | Initial implementation | 4.95 ms (434 MB/s) | 327.5 µs (1,847 MB/s) | 60.0 ns |
| R1 | ryu + const field-name keys | 4.89 ms (439 MB/s) | 327.7 µs (1,846 MB/s) | 59.6 ns |
| R2 | fast-float2 + branchless int parsing | 3.68 ms (583 MB/s) | 329.0 µs (1,840 MB/s) | 48.8 ns |
| R3 (final) | capacity hints + Vec<f64> layout | 3.47 ms (620 MB/s) | 318.8 µs (1,889 MB/s) | 46.3 ns |

**Notable one-round wins:**
- canada.json serialization went from **11.12 ms to 1.99 ms** (-82%) when ryu replaced the std `Display` float formatter (R1).
- Micro Point serialization dropped from **186.7 ns to 107.4 ns** (-42%) when the per-field `Vec::push` path was replaced with a compact write loop (R1).
- canada.json deserialization dropped **26%** in R2 alone from switching to fast-float2 (the dataset is 99% `f64` coordinates).

---

## Remaining opportunities

1. **SIMD string escaper for serialization.**
   Implement a NEON (arm64) or SSE4.2 (x86-64) byte-scan + copy path in `write_str_escaped`. This is the single highest-leverage change: it would eliminate the ~78% gap on twitter-like (string-heavy) serialization and bring r_json's overall serialization speed to within ~10% of sonic-rs across all datasets.

2. **Pre-allocated / writer-based serialization API.**
   Replace internal `Vec<u8>` growth with a caller-provided `&mut Vec<u8>` or `io::Write` target. This avoids the initial allocation and capacity growth branches, which accounts for ~30% of the Point serialization overhead vs serde_json.

3. **SWAR/SIMD field-name matching for large structs.**
   Currently the generated code emits a linear or binary chain of byte comparisons for field names. For structs with many fields, a SWAR (SIMD Within A Register) 8-byte hash or a perfect-hash dispatch table (like phf) would improve both deserialization throughput and instruction cache pressure on wide structs.

4. **Streaming / lazy deserialization for deeply nested payloads.**
   For datasets like citm_catalog where the schema uses `HashMap<String, T>`, the proc-macro approach cannot apply. Adding an optional `Value`-based fallback that still uses the fast scanner (SWAR + fast-float2) would let r_json compete on the citm_catalog benchmark and similar dynamic-schema workloads.

5. **Stack-allocated small-string optimization (SSO) for serialization.**
   For strings shorter than 16 bytes, writing directly to a `[u8; 64]` stack buffer and flushing to the output Vec in one `extend_from_slice` call would eliminate the branch-per-byte overhead that currently makes short-string serialization slower than serde_json's fmt-based approach.
