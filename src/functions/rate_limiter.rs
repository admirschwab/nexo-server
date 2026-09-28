use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

// So oft wird die Tabelle nach IP-Adressen durchsucht, die vergessen werden können
pub const PRUNE_INTERVAL: Duration = Duration::from_secs(30);

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

// Ein Token Bucket pro IP-Adresse.
// IP-Adressen liegen nur im Arbeitsspeicher und werden vergessen, sobald ihr
// Bucket wieder voll ist, also sobald sie für das Limit keine Rolle mehr spielen.
pub struct IpRateLimiter {
    burst: u32,
    per_second: f64,
    table: Mutex<IpTable>,
}

struct IpTable {
    limiters: HashMap<IpAddr, RateLimiter>,
}

impl IpRateLimiter {
    pub fn new(burst: u32, per_second: f64) -> Self {
        Self {
            burst,
            per_second,
            table: Mutex::new(IpTable {
                limiters: HashMap::new(),
            }),
        }
    }

    pub fn try_acquire(&self, ip: IpAddr) -> bool {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .limiters
            .entry(ip)
            .or_insert_with(|| RateLimiter::new(self.burst, self.per_second))
            .try_acquire()
    }

    // Vergisst alle IP-Adressen, deren Bucket wieder voll ist.
    // Wird regelmäßig im Hintergrund aufgerufen (siehe main.rs).
    pub fn prune(&self) {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .limiters
            .retain(|_, limiter| !limiter.is_idle());
    }
}
