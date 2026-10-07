//! Fixed-capacity per-op latency sample buffer (S07 slice48, #21, REVIEW #533 C1).
//!
//! The scheduler trace is an id prefix with no timestamps, so it cannot
//! carry tails. This buffer holds up to [`SAMPLE_CAP`] raw deltas (e.g.
//! guest `rdtsc` cycles around `switch_context`) and reduces them to
//! min / median / p99 / max. No wrap: samples past capacity are counted in
//! `dropped` (never silently overwrite the early population). `core`-only.

/// Sample capacity: enough for a nearest-rank p99 over >= 1000 ops.
pub const SAMPLE_CAP: usize = 1024;

/// Minimum population for a meaningful p99 (REVIEW #533 C1).
pub const MIN_SAMPLES_FOR_P99: usize = 1000;

/// Reduced statistics over the recorded population.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyStats {
    /// Recorded samples.
    pub count: usize,
    /// Samples refused after capacity.
    pub dropped: u64,
    /// Smallest sample.
    pub min: u64,
    /// Lower median.
    pub median: u64,
    /// Nearest-rank 99th percentile.
    pub p99: u64,
    /// Largest sample.
    pub max: u64,
    /// `true` only when `count >= MIN_SAMPLES_FOR_P99`; otherwise `p99`
    /// is reported but must not be published as a tail.
    pub p99_valid: bool,
}

/// Fixed-capacity sample buffer.
#[derive(Debug, Clone)]
pub struct LatencySamples {
    buf: [u64; SAMPLE_CAP],
    len: usize,
    dropped: u64,
    reduced: bool,
}

impl Default for LatencySamples {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencySamples {
    /// Empty buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: [0; SAMPLE_CAP],
            len: 0,
            dropped: 0,
            reduced: false,
        }
    }

    /// Record one sample; returns `false` (and counts a drop) when full.
    /// Also returns `false` (no drop counted) once [`Self::reduce`] has run:
    /// reduction sorts in place, so the buffer is terminal (REVIEW #537 N2).
    pub fn record(&mut self, delta: u64) -> bool {
        if self.reduced {
            return false;
        }
        if self.len < SAMPLE_CAP {
            self.buf[self.len] = delta;
            self.len += 1;
            true
        } else {
            self.dropped = self.dropped.saturating_add(1);
            false
        }
    }

    /// Recorded sample count.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// `true` when nothing is recorded.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Samples refused after capacity.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// `true` once [`Self::reduce`] has run; further `record` calls are refused.
    #[must_use]
    pub const fn is_reduced(&self) -> bool {
        self.reduced
    }

    /// Sort the recorded population in place and reduce it. `None` when
    /// empty. Terminal: later `record` calls return `false`. Median is the lower median; p99 is nearest-rank
    /// (`ceil(0.99 * n)`-th smallest).
    pub fn reduce(&mut self) -> Option<LatencyStats> {
        self.reduced = true;
        let n = self.len;
        if n == 0 {
            return None;
        }
        let s = &mut self.buf[..n];
        s.sort_unstable();
        let rank = (n * 99).div_ceil(100);
        Some(LatencyStats {
            count: n,
            dropped: self.dropped,
            min: s[0],
            median: s[(n - 1) / 2],
            p99: s[rank - 1],
            max: s[n - 1],
            p99_valid: n >= MIN_SAMPLES_FOR_P99,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_reduces_to_none() {
        let mut s = LatencySamples::new();
        assert!(s.is_empty());
        assert_eq!(s.reduce(), None);
    }

    #[test]
    fn single_sample_all_equal_p99_invalid() {
        let mut s = LatencySamples::new();
        assert!(s.record(42));
        let r = s.reduce().unwrap();
        assert_eq!((r.min, r.median, r.p99, r.max), (42, 42, 42, 42));
        assert_eq!(r.count, 1);
        assert!(!r.p99_valid);
    }

    #[test]
    fn thousand_ascending_nearest_rank() {
        let mut s = LatencySamples::new();
        for v in (1..=1000u64).rev() {
            assert!(s.record(v));
        }
        let r = s.reduce().unwrap();
        assert_eq!(r.min, 1);
        assert_eq!(r.median, 500);
        assert_eq!(r.p99, 990);
        assert_eq!(r.max, 1000);
        assert!(r.p99_valid);
    }

    #[test]
    fn tail_outliers_visible_in_p99_not_median() {
        let mut s = LatencySamples::new();
        for _ in 0..980 {
            s.record(100);
        }
        for _ in 0..20 {
            s.record(10_000);
        }
        let r = s.reduce().unwrap();
        assert_eq!(r.median, 100);
        assert_eq!(r.p99, 10_000);
        assert_eq!(r.max, 10_000);
    }

    #[test]
    fn nearest_rank_boundary_at_one_percent_outliers() {
        for outliers in [9, 10, 11] {
            let mut s = LatencySamples::new();
            for _ in 0..(1000 - outliers) {
                assert!(s.record(100));
            }
            for _ in 0..outliers {
                assert!(s.record(10_000));
            }
            let r = s.reduce().unwrap();
            assert!(r.p99_valid);
            assert_eq!(r.p99, if outliers == 11 { 10_000 } else { 100 });
        }
    }

    #[test]
    fn record_after_reduce_is_refused() {
        let mut s = LatencySamples::new();
        assert!(s.record(5));
        assert!(!s.is_reduced());
        let first = s.reduce().unwrap();
        assert!(s.is_reduced());
        assert!(!s.record(1));
        assert_eq!(s.len(), 1);
        assert_eq!(s.dropped(), 0);
        assert_eq!(s.reduce(), Some(first));
    }

    #[test]
    fn overflow_counts_drops_never_wraps() {
        let mut s = LatencySamples::new();
        for i in 0..SAMPLE_CAP as u64 {
            assert!(s.record(i));
        }
        assert!(!s.record(u64::MAX));
        assert!(!s.record(u64::MAX));
        assert_eq!(s.len(), SAMPLE_CAP);
        assert_eq!(s.dropped(), 2);
        let r = s.reduce().unwrap();
        assert_eq!(r.max, SAMPLE_CAP as u64 - 1);
        assert_eq!(r.dropped, 2);
    }
}
