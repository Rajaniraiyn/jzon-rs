//! Hand-written SIMD via `std::arch` intrinsics — the `unsafe` carve-out.
//!
//! aarch64 NEON is baseline on ARMv8 (no runtime detection). x86_64 SSE2 is
//! baseline; AVX2 is probed once via `is_x86_feature_detected!` and cached.

#![allow(unsafe_code)]

// ============================================================================
// aarch64 NEON
// ============================================================================

#[cfg(all(feature = "simd-intrinsics", target_arch = "aarch64"))]
pub mod neon {
    use core::arch::aarch64::*;

    /// 16-byte NEON kernel: find first `"` or `\` in `input[start..]`.
    /// Returns `input.len()` if none. Tail handled by u128 SWAR fallback.
    ///
    /// Uses the `vshrn_n_u16` 16→8 narrow trick to extract a 64-bit nibble
    /// mask from a 16-lane comparison result (4 bits per lane). Same trick
    /// simdjson / simd-json use on aarch64.
    #[inline]
    pub fn find_quote_or_backslash_16(input: &[u8], start: usize) -> usize {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = vdupq_n_u8(b'"');
            let slash = vdupq_n_u8(b'\\');

            while i + 16 <= len {
                let chunk = vld1q_u8(ptr.add(i));
                let m_q = vceqq_u8(chunk, quote);
                let m_s = vceqq_u8(chunk, slash);
                let m = vorrq_u8(m_q, m_s);
                // Reinterpret 16×u8 (0xff/0x00) as 8×u16, shift-right-narrow
                // by 4 → 8×u4 packed into u64. Match nibble = 0xf, miss = 0x0.
                let narrowed = vshrn_n_u16::<4>(vreinterpretq_u16_u8(m));
                let bits = vget_lane_u64::<0>(vreinterpret_u64_u8(narrowed));
                if bits != 0 {
                    return i + (bits.trailing_zeros() / 4) as usize;
                }
                i += 16;
            }
        }

        // Tail: <16 bytes left. Use u64 SWAR from the safe module.
        crate::simd::find_quote_or_backslash(input, i)
    }

    /// 64-byte NEON kernel: 4×16B chunks per loop, OR-reduce then narrow once.
    /// Better instruction-level parallelism on M-series wide cores.
    #[inline]
    pub fn find_quote_or_backslash_64(input: &[u8], start: usize) -> usize {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = vdupq_n_u8(b'"');
            let slash = vdupq_n_u8(b'\\');

            while i + 64 <= len {
                let c0 = vld1q_u8(ptr.add(i));
                let c1 = vld1q_u8(ptr.add(i + 16));
                let c2 = vld1q_u8(ptr.add(i + 32));
                let c3 = vld1q_u8(ptr.add(i + 48));

                let m0 = vorrq_u8(vceqq_u8(c0, quote), vceqq_u8(c0, slash));
                let m1 = vorrq_u8(vceqq_u8(c1, quote), vceqq_u8(c1, slash));
                let m2 = vorrq_u8(vceqq_u8(c2, quote), vceqq_u8(c2, slash));
                let m3 = vorrq_u8(vceqq_u8(c3, quote), vceqq_u8(c3, slash));

                // Cheap any-match probe: OR all four, check for non-zero via max.
                let any = vorrq_u8(vorrq_u8(m0, m1), vorrq_u8(m2, m3));
                if vmaxvq_u8(any) == 0 {
                    i += 64;
                    continue;
                }

                // Found in this 64B window — fall through to 16B for exact index.
                return find_quote_or_backslash_16(input, i);
            }
        }

        find_quote_or_backslash_16(input, i)
    }

    /// 64-byte NEON kernel for `find_escape` — 4×16B with OR-reduce probe.
    #[inline]
    pub fn find_escape_64(input: &[u8], start: usize) -> usize {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = vdupq_n_u8(b'"');
            let slash = vdupq_n_u8(b'\\');
            let ctrl = vdupq_n_u8(0x20);

            while i + 64 <= len {
                let c0 = vld1q_u8(ptr.add(i));
                let c1 = vld1q_u8(ptr.add(i + 16));
                let c2 = vld1q_u8(ptr.add(i + 32));
                let c3 = vld1q_u8(ptr.add(i + 48));

                let m0 = vorrq_u8(vorrq_u8(vceqq_u8(c0, quote), vceqq_u8(c0, slash)), vcltq_u8(c0, ctrl));
                let m1 = vorrq_u8(vorrq_u8(vceqq_u8(c1, quote), vceqq_u8(c1, slash)), vcltq_u8(c1, ctrl));
                let m2 = vorrq_u8(vorrq_u8(vceqq_u8(c2, quote), vceqq_u8(c2, slash)), vcltq_u8(c2, ctrl));
                let m3 = vorrq_u8(vorrq_u8(vceqq_u8(c3, quote), vceqq_u8(c3, slash)), vcltq_u8(c3, ctrl));

                let any = vorrq_u8(vorrq_u8(m0, m1), vorrq_u8(m2, m3));
                if vmaxvq_u8(any) == 0 {
                    i += 64;
                    continue;
                }
                return find_escape_16(input, i);
            }
        }

        find_escape_16(input, i)
    }

    /// 16-byte NEON kernel for `find_escape`: `"`, `\`, or byte < 0x20.
    #[inline]
    pub fn find_escape_16(input: &[u8], start: usize) -> usize {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = vdupq_n_u8(b'"');
            let slash = vdupq_n_u8(b'\\');
            let ctrl = vdupq_n_u8(0x20);

            while i + 16 <= len {
                let chunk = vld1q_u8(ptr.add(i));
                let m_q = vceqq_u8(chunk, quote);
                let m_s = vceqq_u8(chunk, slash);
                let m_c = vcltq_u8(chunk, ctrl);
                let m = vorrq_u8(vorrq_u8(m_q, m_s), m_c);
                let narrowed = vshrn_n_u16::<4>(vreinterpretq_u16_u8(m));
                let bits = vget_lane_u64::<0>(vreinterpret_u64_u8(narrowed));
                if bits != 0 {
                    return i + (bits.trailing_zeros() / 4) as usize;
                }
                i += 16;
            }
        }

        // Tail: scalar walk (u128-SWAR fallback lives in simd.rs but is gated
        // on `feature = "simd"`; we already require it transitively).
        let mut j = i;
        while j < len {
            let b = input[j];
            if b == b'"' || b == b'\\' || b < 0x20 {
                return j;
            }
            j += 1;
        }
        len
    }

    /// Fused scan for `"`, `\`, control (< 0x20), and ASCII-only tracking.
    #[inline]
    pub fn scan_string_run_64(input: &[u8], start: usize) -> (usize, bool) {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();
        let mut ascii_only = true;

        unsafe {
            let quote = vdupq_n_u8(b'"');
            let slash = vdupq_n_u8(b'\\');
            let ctrl = vdupq_n_u8(0x20);
            let high = vdupq_n_u8(0x80);

            while i + 64 <= len {
                let c0 = vld1q_u8(ptr.add(i));
                let c1 = vld1q_u8(ptr.add(i + 16));
                let c2 = vld1q_u8(ptr.add(i + 32));
                let c3 = vld1q_u8(ptr.add(i + 48));

                let m0 = vorrq_u8(vorrq_u8(vceqq_u8(c0, quote), vceqq_u8(c0, slash)), vcltq_u8(c0, ctrl));
                let m1 = vorrq_u8(vorrq_u8(vceqq_u8(c1, quote), vceqq_u8(c1, slash)), vcltq_u8(c1, ctrl));
                let m2 = vorrq_u8(vorrq_u8(vceqq_u8(c2, quote), vceqq_u8(c2, slash)), vcltq_u8(c2, ctrl));
                let m3 = vorrq_u8(vorrq_u8(vceqq_u8(c3, quote), vceqq_u8(c3, slash)), vcltq_u8(c3, ctrl));

                let any = vorrq_u8(vorrq_u8(m0, m1), vorrq_u8(m2, m3));
                if vmaxvq_u8(any) == 0 {
                    let any_high = vorrq_u8(
                        vorrq_u8(vandq_u8(c0, high), vandq_u8(c1, high)),
                        vorrq_u8(vandq_u8(c2, high), vandq_u8(c3, high)),
                    );
                    if vmaxvq_u8(any_high) != 0 {
                        ascii_only = false;
                    }
                    i += 64;
                    continue;
                }
                return scan_string_run_16(input, i, ascii_only);
            }
        }

        scan_string_run_16(input, i, ascii_only)
    }

    #[inline]
    fn scan_string_run_16(input: &[u8], start: usize, mut ascii_only: bool) -> (usize, bool) {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = vdupq_n_u8(b'"');
            let slash = vdupq_n_u8(b'\\');
            let ctrl = vdupq_n_u8(0x20);
            let high = vdupq_n_u8(0x80);

            while i + 16 <= len {
                let chunk = vld1q_u8(ptr.add(i));
                let m_q = vceqq_u8(chunk, quote);
                let m_s = vceqq_u8(chunk, slash);
                let m_c = vcltq_u8(chunk, ctrl);
                let m = vorrq_u8(vorrq_u8(m_q, m_s), m_c);
                let narrowed = vshrn_n_u16::<4>(vreinterpretq_u16_u8(m));
                let bits = vget_lane_u64::<0>(vreinterpret_u64_u8(narrowed));
                if bits != 0 {
                    let stop = i + (bits.trailing_zeros() / 4) as usize;
                    return (
                        stop,
                        ascii_only && input[start..stop].iter().all(|&b| b.is_ascii()),
                    );
                }
                if vmaxvq_u8(vandq_u8(chunk, high)) != 0 {
                    ascii_only = false;
                }
                i += 16;
            }
        }

        let (stop, tail_ascii) = crate::simd::scan_string_run_scalar(input, i);
        (stop, ascii_only && tail_ascii)
    }
}

// ============================================================================
// x86_64 SSE2 (baseline) / AVX2 (runtime-detected)
// ============================================================================

#[cfg(all(feature = "simd-intrinsics", target_arch = "x86_64"))]
pub mod x86 {
    use core::arch::x86_64::*;
    use core::sync::atomic::{AtomicU8, Ordering};

    // One-shot CPUID cache. 0 = unknown, 1 = AVX2, 2 = SSE2 only.
    static AVX2_TIER: AtomicU8 = AtomicU8::new(0);

    #[inline]
    fn has_avx2() -> bool {
        match AVX2_TIER.load(Ordering::Relaxed) {
            1 => true,
            2 => false,
            _ => {
                let tier = if is_x86_feature_detected!("avx2") { 1 } else { 2 };
                AVX2_TIER.store(tier, Ordering::Relaxed);
                tier == 1
            }
        }
    }

    // --- SSE2 kernels (always available on x86_64) ---

    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn find_q_or_bs_sse2(input: &[u8], start: usize) -> usize {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = _mm_set1_epi8(b'"' as i8);
            let slash = _mm_set1_epi8(b'\\' as i8);

            while i + 16 <= len {
                let chunk = _mm_loadu_si128(ptr.add(i) as *const __m128i);
                let m_q = _mm_cmpeq_epi8(chunk, quote);
                let m_s = _mm_cmpeq_epi8(chunk, slash);
                let m = _mm_or_si128(m_q, m_s);
                let mask = _mm_movemask_epi8(m) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 16;
            }
        }

        crate::simd::find_quote_or_backslash(input, i)
    }

    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn find_escape_sse2(input: &[u8], start: usize) -> usize {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = _mm_set1_epi8(b'"' as i8);
            let slash = _mm_set1_epi8(b'\\' as i8);
            // cmplt with 0x20 as signed: any byte < 0x20 (including 0..0x1f
            // and the high-bit DEL etc. are >= 0x20 unsigned but negative
            // signed — we want NOT those). Use _mm_subs_epu8 trick instead:
            // saturating sub by 0x1f gives 0 iff byte < 0x20 (since 0x1f<0x20
            // unsigned). Then cmpeq with zero vector.
            let ctrl_thresh = _mm_set1_epi8(0x1f);
            let zero = _mm_setzero_si128();

            while i + 16 <= len {
                let chunk = _mm_loadu_si128(ptr.add(i) as *const __m128i);
                let m_q = _mm_cmpeq_epi8(chunk, quote);
                let m_s = _mm_cmpeq_epi8(chunk, slash);
                let m_c = _mm_cmpeq_epi8(_mm_subs_epu8(chunk, ctrl_thresh), zero);
                let m = _mm_or_si128(_mm_or_si128(m_q, m_s), m_c);
                let mask = _mm_movemask_epi8(m) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 16;
            }
        }

        // tail
        let mut j = i;
        while j < len {
            let b = input[j];
            if b == b'"' || b == b'\\' || b < 0x20 {
                return j;
            }
            j += 1;
        }
        len
    }

    // --- AVX2 kernels (gated on cached CPUID probe) ---

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn find_q_or_bs_avx2(input: &[u8], start: usize) -> usize {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = _mm256_set1_epi8(b'"' as i8);
            let slash = _mm256_set1_epi8(b'\\' as i8);

            while i + 32 <= len {
                let chunk = _mm256_loadu_si256(ptr.add(i) as *const __m256i);
                let m_q = _mm256_cmpeq_epi8(chunk, quote);
                let m_s = _mm256_cmpeq_epi8(chunk, slash);
                let m = _mm256_or_si256(m_q, m_s);
                let mask = _mm256_movemask_epi8(m) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 32;
            }
        }

        // SAFETY: SSE2 is baseline on x86_64.
        unsafe { find_q_or_bs_sse2(input, i) }
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn find_escape_avx2(input: &[u8], start: usize) -> usize {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = _mm256_set1_epi8(b'"' as i8);
            let slash = _mm256_set1_epi8(b'\\' as i8);
            let ctrl_thresh = _mm256_set1_epi8(0x1f);
            let zero = _mm256_setzero_si256();

            while i + 32 <= len {
                let chunk = _mm256_loadu_si256(ptr.add(i) as *const __m256i);
                let m_q = _mm256_cmpeq_epi8(chunk, quote);
                let m_s = _mm256_cmpeq_epi8(chunk, slash);
                let m_c = _mm256_cmpeq_epi8(_mm256_subs_epu8(chunk, ctrl_thresh), zero);
                let m = _mm256_or_si256(_mm256_or_si256(m_q, m_s), m_c);
                let mask = _mm256_movemask_epi8(m) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 32;
            }
        }

        unsafe { find_escape_sse2(input, i) }
    }

    // --- Public dispatchers — pick best path per cached CPUID probe ---

    /// SSE2-only kernel for benches.
    #[inline]
    pub fn find_quote_or_backslash_16(input: &[u8], start: usize) -> usize {
        unsafe { find_q_or_bs_sse2(input, start) }
    }

    /// AVX2 if available, else SSE2.
    #[inline]
    pub fn find_quote_or_backslash_32(input: &[u8], start: usize) -> usize {
        if has_avx2() {
            unsafe { find_q_or_bs_avx2(input, start) }
        } else {
            unsafe { find_q_or_bs_sse2(input, start) }
        }
    }

    #[inline]
    pub fn find_escape_16(input: &[u8], start: usize) -> usize {
        unsafe { find_escape_sse2(input, start) }
    }

    #[inline]
    pub fn find_escape_32(input: &[u8], start: usize) -> usize {
        if has_avx2() {
            unsafe { find_escape_avx2(input, start) }
        } else {
            unsafe { find_escape_sse2(input, start) }
        }
    }

    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn scan_string_run_sse2(input: &[u8], start: usize, mut ascii_only: bool) -> (usize, bool) {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = _mm_set1_epi8(b'"' as i8);
            let slash = _mm_set1_epi8(b'\\' as i8);
            let ctrl_thresh = _mm_set1_epi8(0x1f);
            let zero = _mm_setzero_si128();
            let high = _mm_set1_epi8(0x80u8 as i8);

            while i + 16 <= len {
                let chunk = _mm_loadu_si128(ptr.add(i) as *const __m128i);
                let m_q = _mm_cmpeq_epi8(chunk, quote);
                let m_s = _mm_cmpeq_epi8(chunk, slash);
                let m_c = _mm_cmpeq_epi8(_mm_subs_epu8(chunk, ctrl_thresh), zero);
                let m = _mm_or_si128(_mm_or_si128(m_q, m_s), m_c);
                let mask = _mm_movemask_epi8(m) as u32;
                if mask != 0 {
                    let stop = i + mask.trailing_zeros() as usize;
                    return (
                        stop,
                        ascii_only && input[start..stop].iter().all(|&b| b.is_ascii()),
                    );
                }
                if _mm_movemask_epi8(_mm_and_si128(chunk, high)) != 0 {
                    ascii_only = false;
                }
                i += 16;
            }
        }

        let (stop, tail_ascii) = crate::simd::scan_string_run_scalar(input, i);
        (stop, ascii_only && tail_ascii)
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn scan_string_run_avx2(input: &[u8], start: usize, mut ascii_only: bool) -> (usize, bool) {
        let mut i = start;
        let len = input.len();
        let ptr = input.as_ptr();

        unsafe {
            let quote = _mm256_set1_epi8(b'"' as i8);
            let slash = _mm256_set1_epi8(b'\\' as i8);
            let ctrl_thresh = _mm256_set1_epi8(0x1f);
            let zero = _mm256_setzero_si256();
            let high = _mm256_set1_epi8(0x80u8 as i8);

            while i + 32 <= len {
                let chunk = _mm256_loadu_si256(ptr.add(i) as *const __m256i);
                let m_q = _mm256_cmpeq_epi8(chunk, quote);
                let m_s = _mm256_cmpeq_epi8(chunk, slash);
                let m_c = _mm256_cmpeq_epi8(_mm256_subs_epu8(chunk, ctrl_thresh), zero);
                let m = _mm256_or_si256(_mm256_or_si256(m_q, m_s), m_c);
                let mask = _mm256_movemask_epi8(m) as u32;
                if mask != 0 {
                    let stop = i + mask.trailing_zeros() as usize;
                    return (
                        stop,
                        ascii_only && input[start..stop].iter().all(|&b| b.is_ascii()),
                    );
                }
                if _mm256_movemask_epi8(_mm256_and_si256(chunk, high)) != 0 {
                    ascii_only = false;
                }
                i += 32;
            }
        }

        unsafe { scan_string_run_sse2(input, i, ascii_only) }
    }

    #[inline]
    pub fn scan_string_run_32(input: &[u8], start: usize) -> (usize, bool) {
        if has_avx2() {
            unsafe { scan_string_run_avx2(input, start, true) }
        } else {
            unsafe { scan_string_run_sse2(input, start, true) }
        }
    }
}
