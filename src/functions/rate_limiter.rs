use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, PoisonError},
    time::Instant,
};

// Ab so vielen Einträgen werden unbenutzte IP-Adressen aus der Tabelle entfernt
const IP_TABLE_PRUNE_THRESHOLD: usize = 1000;

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
    pub fn new(burst: u32, per_second: f64) -> Self {
        Self {
            tokens: burst as f64,
            burst: burst as f64,
            per_second,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        self.tokens = (self.tokens + elapsed * self.per_second).min(self.burst);
        self.last_refill = now;
    }

    pub fn try_acquire(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    // Voll aufgefüllt, also nicht von einem frischen Limiter zu unterscheiden
    fn is_idle(&mut self) -> bool {
        self.refill();
        self.tokens >= self.burst
    }
}

// Ein Token Bucket pro IP-Adresse
pub struct IpRateLimiter {
    burst: u32,
    per_second: f64,
    limiters: Mutex<HashMap<IpAddr, RateLimiter>>,
}

impl IpRateLimiter {
    pub fn new(burst: u32, per_second: f64) -> Self {
        Self {
            burst,
            per_second,
            limiters: Mutex::new(HashMap::new()),
        }
    }

    pub fn try_acquire(&self, ip: IpAddr) -> bool {
        let mut limiters = self
            .limiters
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        if limiters.len() >= IP_TABLE_PRUNE_THRESHOLD {
            limiters.retain(|_, limiter| !limiter.is_idle());
        }

        limiters
            .entry(ip)
            .or_insert_with(|| RateLimiter::new(self.burst, self.per_second))
            .try_acquire()
    }
}
