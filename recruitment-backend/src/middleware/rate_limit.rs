//! Token-bucket rate limiting, per client plus a global ceiling.
//!
//! Each client refills at `rps` tokens per second and may burst up to
//! [`BURST_SECONDS`] worth, so a dashboard page that fires a dozen requests at
//! once (plus its polling) is served while a sustained flood is still cut off.
//! The global bucket bounds total load even if client keys are spoofed.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Burst capacity, in seconds of sustained rate.
const BURST_SECONDS: f64 = 5.0;
const MIN_BURST: f64 = 30.0;
/// The global bucket allows this many times the per-client rate.
const GLOBAL_MULTIPLIER: f64 = 10.0;
const IDLE_EVICTION: Duration = Duration::from_secs(10 * 60);
const MAX_TRACKED_CLIENTS: usize = 10_000;

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    updated: Instant,
}

impl Bucket {
    fn full(capacity: f64, now: Instant) -> Self {
        Self { tokens: capacity, updated: now }
    }

    /// Takes one token, or returns how long until one is available.
    fn take(&mut self, rate: f64, capacity: f64, now: Instant) -> Result<(), Duration> {
        let elapsed = now.saturating_duration_since(self.updated).as_secs_f64();
        self.tokens = (self.tokens + elapsed * rate).min(capacity);
        self.updated = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            Err(Duration::from_secs_f64((1.0 - self.tokens) / rate))
        }
    }
}

#[derive(Debug)]
struct Buckets {
    global: Bucket,
    clients: HashMap<String, Bucket>,
}

#[derive(Clone, Debug)]
pub struct RateLimiter {
    rate: f64,
    burst: f64,
    buckets: Arc<Mutex<Buckets>>,
}

impl RateLimiter {
    fn new(rps: u32) -> Self {
        let rate = f64::from(rps.max(1));
        let burst = (rate * BURST_SECONDS).max(MIN_BURST);
        Self {
            rate,
            burst,
            buckets: Arc::new(Mutex::new(Buckets {
                global: Bucket::full(burst * GLOBAL_MULTIPLIER, Instant::now()),
                clients: HashMap::new(),
            })),
        }
    }

    fn check(&self, client: &str) -> Result<(), Duration> {
        self.check_at(client, Instant::now())
    }

    fn check_at(&self, client: &str, now: Instant) -> Result<(), Duration> {
        let mut buckets = self.buckets.lock().unwrap_or_else(|p| p.into_inner());

        if buckets.clients.len() >= MAX_TRACKED_CLIENTS && !buckets.clients.contains_key(client) {
            buckets
                .clients
                .retain(|_, b| now.saturating_duration_since(b.updated) < IDLE_EVICTION);
            if buckets.clients.len() >= MAX_TRACKED_CLIENTS {
                buckets.clients.clear();
            }
        }

        let (rate, burst) = (self.rate, self.burst);
        buckets
            .clients
            .entry(client.to_string())
            .or_insert_with(|| Bucket::full(burst, now))
            .take(rate, burst, now)?;
        buckets
            .global
            .take(rate * GLOBAL_MULTIPLIER, burst * GLOBAL_MULTIPLIER, now)
    }
}

/// Best-effort client identity. Requests normally arrive through the edge
/// proxy, Caddy and the Next.js rewrite, so the socket peer is a proxy and the
/// forwarded headers are the only hint of the real caller.
fn client_key(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    let header_ip = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    header_ip("x-real-ip")
        .or_else(|| header_ip("x-forwarded-for"))
        .or_else(|| peer.map(|p| p.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn rps_middleware(
    State(state): State<RateLimiter>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| *addr);
    let client = client_key(req.headers(), peer);

    if let Err(wait) = state.check(&client) {
        tracing::warn!("Rate limit hit for {} on {}", client, req.uri().path());
        let retry_after = wait.as_secs().max(1).to_string();
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, retry_after)],
            "rate_limit_exceeded",
        )
            .into_response();
    }
    next.run(req).await
}

pub fn new_rps_state(rps: u32) -> RateLimiter {
    RateLimiter::new(rps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_load_burst_is_allowed() {
        let limiter = RateLimiter::new(10);
        let now = Instant::now();
        for i in 0..50 {
            assert!(limiter.check_at("1.2.3.4", now).is_ok(), "request {} rejected", i);
        }
        assert!(limiter.check_at("1.2.3.4", now).is_err());
    }

    #[test]
    fn tokens_refill_at_the_configured_rate() {
        let limiter = RateLimiter::new(10);
        let start = Instant::now();
        while limiter.check_at("a", start).is_ok() {}

        let wait = limiter.check_at("a", start).unwrap_err();
        assert!(wait <= Duration::from_millis(101));
        let later = start + Duration::from_millis(150);
        assert!(limiter.check_at("a", later).is_ok());
        assert!(limiter.check_at("a", later).is_err());
    }

    #[test]
    fn one_busy_client_does_not_lock_out_another() {
        let limiter = RateLimiter::new(10);
        let now = Instant::now();
        while limiter.check_at("busy", now).is_ok() {}
        assert!(limiter.check_at("quiet", now).is_ok());
    }

    #[test]
    fn the_global_ceiling_holds_across_clients() {
        let limiter = RateLimiter::new(1);
        let now = Instant::now();
        let allowed = (0..2_000)
            .filter(|i| limiter.check_at(&format!("client-{}", i), now).is_ok())
            .count();
        assert_eq!(allowed as f64, MIN_BURST * GLOBAL_MULTIPLIER);
    }

    #[test]
    fn client_key_prefers_forwarded_headers() {
        let peer: SocketAddr = "172.18.0.5:41000".parse().unwrap();
        let mut headers = HeaderMap::new();
        assert_eq!(client_key(&headers, Some(peer)), "172.18.0.5");

        headers.insert("x-forwarded-for", "203.0.113.7, 172.18.0.3".parse().unwrap());
        assert_eq!(client_key(&headers, Some(peer)), "203.0.113.7");

        headers.insert("x-real-ip", "198.51.100.2".parse().unwrap());
        assert_eq!(client_key(&headers, Some(peer)), "198.51.100.2");

        assert_eq!(client_key(&HeaderMap::new(), None), "unknown");
    }
}
