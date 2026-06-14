//! Per-parse statistics collected when the `stats` feature is enabled.
//!
//! Enabled at compile time — zero cost when the feature is off.

/// Accumulated statistics from a single `FromJson` parse run.
#[derive(Debug, Clone, Default)]
pub struct ScannerStats {
    /// Number of string values that were returned as zero-copy `&'de str`
    /// borrows (no heap allocation required).
    pub zero_copy_borrows: u64,
    /// Number of string values that required heap allocation because they
    /// contained JSON escape sequences.
    pub heap_allocations: u64,
    /// Total bytes consumed by the scanner.
    pub bytes_scanned: u64,
    /// Number of times the field-dispatch hint cache produced a correct
    /// prediction (i.e., the hinted field matched the incoming key).
    pub hint_hits: u64,
    /// Number of times the hint cache missed and a full dispatch was needed.
    pub hint_misses: u64,
}

impl ScannerStats {
    /// Hit rate of the field-dispatch hint cache: 0.0 → 1.0.
    /// Returns `None` when no dispatches have been recorded.
    pub fn hint_hit_rate(&self) -> Option<f64> {
        let total = self.hint_hits + self.hint_misses;
        if total == 0 { None } else { Some(self.hint_hits as f64 / total as f64) }
    }

    /// Fraction of parsed string values that were zero-copy borrows: 0.0 → 1.0.
    pub fn zero_copy_rate(&self) -> Option<f64> {
        let total = self.zero_copy_borrows + self.heap_allocations;
        if total == 0 { None } else { Some(self.zero_copy_borrows as f64 / total as f64) }
    }
}
