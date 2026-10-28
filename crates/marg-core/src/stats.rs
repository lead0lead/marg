//! Rolling timing statistics for the debug overlay.

use std::collections::VecDeque;

/// The last `capacity` samples (e.g. frame times in ms).
#[derive(Debug, Clone)]
pub struct Rolling {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl Rolling {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
        }
    }

    pub fn push(&mut self, v: f32) {
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(v);
    }

    pub fn last(&self) -> Option<f32> {
        self.samples.back().copied()
    }

    /// The `q` quantile (0.0–1.0), nearest-rank.
    pub fn quantile(&self, q: f32) -> Option<f32> {
        if self.samples.is_empty() {
            return None;
        }
        let mut v: Vec<f32> = self.samples.iter().copied().collect();
        v.sort_by(f32::total_cmp);
        let rank = ((q.clamp(0.0, 1.0) * v.len() as f32).ceil() as usize).max(1);
        Some(v[rank - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_last_samples_and_quantiles() {
        let mut r = Rolling::new(100);
        assert_eq!(r.quantile(0.95), None);
        for i in 1..=200 {
            r.push(i as f32);
        }
        assert_eq!(r.last(), Some(200.0));
        // Samples 101..=200; the 95th percentile is the 95th of them.
        assert_eq!(r.quantile(0.95), Some(195.0));
        assert_eq!(r.quantile(0.0), Some(101.0));
        assert_eq!(r.quantile(1.0), Some(200.0));
    }
}
