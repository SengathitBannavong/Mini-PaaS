use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Configuration for the rate limiter
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Maximum tokens (burst capacity)
    pub max_tokens: u32,
    /// Tokens added per second
    pub refill_rate: f64,
}

impl RateLimitConfig {
    pub fn new(max_tokens: u32, refill_rate: f64) -> Self {
        RateLimitConfig {
            max_tokens,
            refill_rate,
        }
    }
}

/// A single client's token bucket
struct TokenBucket {
    tokens: f64,
    last_refill: std::time::Instant,
    max_tokens: u32,
    refill_rate: f64,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        TokenBucket {
            tokens: config.max_tokens as f64,
            last_refill: std::time::Instant::now(),
            max_tokens: config.max_tokens,
            refill_rate: config.refill_rate,
        }
    }

    /// Try to consume a token. Returns true if allowed, false if rate limited.
    fn try_consume(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Add tokens based on elapsed time
    fn refill(&mut self) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens as f64);
        self.last_refill = now;
    }

    fn available_tokens(&self) -> u32 {
        self.tokens as u32
    }
}

/// Stores token buckets for all clients
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, TokenBucket>>,
    config: RateLimitConfig,
    last_sweep: Mutex<std::time::Instant>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        RateLimiter {
            buckets: Mutex::new(HashMap::new()),
            config,
            last_sweep: Mutex::new(std::time::Instant::now()),
        }
    }

    /// Check & refil token if a request from the given key should be allowed
    pub fn check(&self, key: &str) -> RateLimitResult {
        let mut buckets = self.buckets.lock().unwrap();
        self.sweep(&mut buckets);
        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(&self.config));

        if bucket.try_consume() {
            RateLimitResult::Allowed
        } else {
            RateLimitResult::Limited {
                retry_after_secs: ((1.0 - bucket.tokens) / self.config.refill_rate)
                    .ceil()
                    .max(1.0) as u32,
            }
        }
    }

    /// Peek token if a request from a given key should be allowed
    pub fn peek(&self, key: &str) -> u32 {
        let mut buckets = self.buckets.lock().unwrap();

        buckets.get_mut(key).map_or(self.config.max_tokens, |b| {
            b.refill();
            b.available_tokens()
        })
    }

    pub fn limit(&self) -> u32 {
        self.config.max_tokens
    }

    /// Drop buckets idle long enough to be full again - identical to a fresh bucket
    fn sweep(&self, buckets: &mut HashMap<String, TokenBucket>) {
        let mut last = self.last_sweep.lock().unwrap();
        if last.elapsed() < Duration::from_secs(60) {
            return;
        }
        *last = Instant::now();
        let full_after = self.config.max_tokens as f64 / self.config.refill_rate;
        // OPTIMIZE: too long to scan on map
        buckets.retain(|_, b| b.last_refill.elapsed().as_secs_f64() < full_after);
    }
}

pub enum RateLimitResult {
    Allowed,
    Limited { retry_after_secs: u32 },
}
