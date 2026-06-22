use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct LoginGuard {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
    max_attempts: u32,
    lockout: Duration,
    window: Duration,
}

struct Entry {
    failures: u32,
    first_failure: Instant,
    locked_until: Option<Instant>,
}

impl LoginGuard {
    pub fn new(max_attempts: u32, lockout_secs: u64, window_secs: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max_attempts: max_attempts.max(1),
            lockout: Duration::from_secs(lockout_secs),
            window: Duration::from_secs(window_secs),
        }
    }

    pub fn check(&self, key: &str) -> Result<(), u64> {
        let mut map = self.inner.lock().expect("login guard mutex poisoned");
        if let Some(entry) = map.get(key) {
            if let Some(until) = entry.locked_until {
                let now = Instant::now();
                if now < until {
                    return Err((until - now).as_secs() + 1);
                }
                map.remove(key);
            }
        }
        Ok(())
    }

    pub fn record_failure(&self, key: &str) -> u32 {
        let mut map = self.inner.lock().expect("login guard mutex poisoned");
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert(Entry {
            failures: 0,
            first_failure: now,
            locked_until: None,
        });

        if now.duration_since(entry.first_failure) > self.window {
            entry.failures = 0;
            entry.first_failure = now;
            entry.locked_until = None;
        }

        entry.failures += 1;
        if entry.failures >= self.max_attempts {
            entry.locked_until = Some(now + self.lockout);
            return 0;
        }
        self.max_attempts - entry.failures
    }

    pub fn record_success(&self, key: &str) {
        self.inner
            .lock()
            .expect("login guard mutex poisoned")
            .remove(key);
    }
}

impl Default for LoginGuard {
    fn default() -> Self {
        Self::new(5, 15 * 60, 15 * 60)
    }
}
