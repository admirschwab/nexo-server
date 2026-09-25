use std::time::Instant;

// Token Bucket: Jede Aktion kostet ein Token. Das Guthaben füllt sich mit
// `per_second` Tokens pro Sekunde wieder auf, höchstens bis `burst`.
// So sind kurze Schübe erlaubt, dauerhaftes Fluten aber nicht.
pub struct RateLimiter {
    tokens: f64,
    burst: f64,
    per_second: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(burst: u32, per_second: u32) -> Self {
        Self {
            tokens: burst as f64,
            burst: burst as f64,
            per_second: per_second as f64,
            last_refill: Instant::now(),
        }
    }

    pub fn try_acquire(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        self.tokens = (self.tokens + elapsed * self.per_second).min(self.burst);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}
