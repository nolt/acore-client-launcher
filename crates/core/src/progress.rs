use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// What the updater is doing, for the status line and progress bar.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    FetchingManifest,
    Checking {
        done: usize,
        total: usize,
    },
    Downloading {
        /// 1-based: files finished + 1 (capped at `files_total`).
        file: usize,
        files_total: usize,
        bytes_done: u64,
        bytes_total: u64,
        /// Manifest path of the longest-running download in progress.
        current: String,
        /// Averaged over the last few seconds; 0 until there is enough data.
        bytes_per_sec: f64,
        /// Time left for the whole update, once the speed is known.
        eta: Option<Duration>,
    },
}

/// Download speed averaged over a sliding window, so the number does not jump
/// with every network packet.
#[derive(Debug)]
pub struct RateMeter {
    window: Duration,
    samples: VecDeque<(Instant, u64)>,
}

impl RateMeter {
    pub fn new(window: Duration) -> Self {
        RateMeter {
            window,
            samples: VecDeque::new(),
        }
    }

    /// `total` is the running count of bytes received so far.
    pub fn record(&mut self, now: Instant, total: u64) {
        self.samples.push_back((now, total));
        while self.samples.len() > 2 && now.duration_since(self.samples[0].0) > self.window {
            self.samples.pop_front();
        }
    }

    pub fn bytes_per_sec(&self) -> f64 {
        let (Some(first), Some(last)) = (self.samples.front(), self.samples.back()) else {
            return 0.0;
        };
        let elapsed = last.0.duration_since(first.0).as_secs_f64();
        if elapsed < 0.5 {
            return 0.0;
        }
        (last.1 - first.1) as f64 / elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_over_the_window() {
        let start = Instant::now();
        let mut meter = RateMeter::new(Duration::from_secs(5));
        meter.record(start, 0);
        assert_eq!(meter.bytes_per_sec(), 0.0);
        for s in 1..=10 {
            meter.record(start + Duration::from_secs(s), s * 1000);
        }
        assert!((meter.bytes_per_sec() - 1000.0).abs() < 1e-9);
        // A burst only moves the average, it does not replace it.
        meter.record(start + Duration::from_secs(11), 16_000);
        assert!(meter.bytes_per_sec() < 3000.0);
    }
}
