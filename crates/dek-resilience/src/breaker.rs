//! breaker.rs — per-evaluator circuit breaker.
//!
//! When a PDP/evaluator fails (error or timeout) repeatedly, the breaker OPENS
//! and short-circuits subsequent calls for a cooldown — the caller then FAILS
//! CLOSED (deny) immediately instead of queueing behind a sick backend. After
//! cooldown it goes HALF-OPEN and admits a few probes; success closes it again.
//!
//! The transition core takes an explicit `now: Instant` so it is deterministic
//! to unit test; public wrappers use `Instant::now()`.

use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct CircuitConfig {
    /// Consecutive failures (in Closed) that trip the breaker open.
    pub failure_threshold: u32,
    /// How long to stay Open before allowing half-open probes.
    pub cooldown: Duration,
    /// Successful probes needed (in HalfOpen) to close again.
    pub half_open_required_successes: u32,
}

impl Default for CircuitConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            cooldown: Duration::from_secs(10),
            half_open_required_successes: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Closed,
    Open,
    HalfOpen,
}

struct Inner {
    state: State,
    consecutive_failures: u32,
    opened_at: Option<Instant>,
    half_open_successes: u32,
    half_open_inflight: u32,
}

pub struct CircuitBreaker {
    name: String,
    cfg: CircuitConfig,
    inner: Mutex<Inner>,
}

/// Whether a call may proceed right now.
#[derive(Debug, PartialEq)]
pub enum Admit {
    Allow,
    Reject, // breaker open -> caller must fail closed
}

impl CircuitBreaker {
    pub fn new(name: impl Into<String>, cfg: CircuitConfig) -> Self {
        Self {
            name: name.into(),
            cfg,
            inner: Mutex::new(Inner {
                state: State::Closed,
                consecutive_failures: 0,
                opened_at: None,
                half_open_successes: 0,
                half_open_inflight: 0,
            }),
        }
    }

    pub fn permitted(&self) -> Admit {
        self.permitted_at(Instant::now())
    }

    /// Testable core.
    pub fn permitted_at(&self, now: Instant) -> Admit {
        let mut g = self.inner.lock().unwrap();
        match g.state {
            State::Closed => Admit::Allow,
            State::Open => {
                let elapsed = g.opened_at.map(|t| now.duration_since(t)).unwrap_or_default();
                if elapsed >= self.cfg.cooldown {
                    // move to half-open, admit a single probe
                    g.state = State::HalfOpen;
                    g.half_open_successes = 0;
                    g.half_open_inflight = 1;
                    Admit::Allow
                } else {
                    Admit::Reject
                }
            }
            State::HalfOpen => {
                // admit limited probes (one at a time keeps it simple + safe)
                if g.half_open_inflight == 0 {
                    g.half_open_inflight = 1;
                    Admit::Allow
                } else {
                    Admit::Reject
                }
            }
        }
    }

    pub fn on_success(&self) {
        let mut g = self.inner.lock().unwrap();
        match g.state {
            State::Closed => {
                g.consecutive_failures = 0;
            }
            State::HalfOpen => {
                g.half_open_inflight = g.half_open_inflight.saturating_sub(1);
                g.half_open_successes += 1;
                if g.half_open_successes >= self.cfg.half_open_required_successes {
                    g.state = State::Closed;
                    g.consecutive_failures = 0;
                    g.opened_at = None;
                }
            }
            State::Open => {}
        }
        metrics::gauge!("dek_circuit_open", "evaluator" => self.name.clone())
            .set(if g.state == State::Open { 1.0 } else { 0.0 });
    }

    pub fn on_failure(&self) {
        self.on_failure_at(Instant::now())
    }

    pub fn on_failure_at(&self, now: Instant) {
        let mut g = self.inner.lock().unwrap();
        match g.state {
            State::Closed => {
                g.consecutive_failures += 1;
                if g.consecutive_failures >= self.cfg.failure_threshold {
                    g.state = State::Open;
                    g.opened_at = Some(now);
                    warn!("circuit '{}' OPEN after {} failures", self.name, g.consecutive_failures);
                    metrics::counter!("dek_circuit_open_total", "evaluator" => self.name.clone()).increment(1);
                }
            }
            State::HalfOpen => {
                // probe failed -> back to open
                g.state = State::Open;
                g.opened_at = Some(now);
                g.half_open_inflight = 0;
                g.half_open_successes = 0;
                warn!("circuit '{}' re-OPEN (half-open probe failed)", self.name);
            }
            State::Open => {}
        }
        metrics::gauge!("dek_circuit_open", "evaluator" => self.name.clone())
            .set(if g.state == State::Open { 1.0 } else { 0.0 });
    }

    pub fn is_open(&self) -> bool {
        self.inner.lock().unwrap().state == State::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> CircuitConfig {
        CircuitConfig { failure_threshold: 3, cooldown: Duration::from_secs(10), half_open_required_successes: 2 }
    }

    #[test]
    fn opens_after_threshold_then_rejects() {
        let b = CircuitBreaker::new("pdp", cfg());
        let t0 = Instant::now();
        assert_eq!(b.permitted_at(t0), Admit::Allow);
        for _ in 0..3 {
            b.on_failure_at(t0);
        }
        assert!(b.is_open());
        assert_eq!(b.permitted_at(t0), Admit::Reject); // within cooldown
    }

    #[test]
    fn half_open_after_cooldown_then_closes_on_success() {
        let b = CircuitBreaker::new("pdp", cfg());
        let t0 = Instant::now();
        for _ in 0..3 {
            b.on_failure_at(t0);
        }
        assert!(b.is_open());
        // after cooldown -> half-open admits a probe
        let t1 = t0 + Duration::from_secs(11);
        assert_eq!(b.permitted_at(t1), Admit::Allow);
        assert_eq!(b.permitted_at(t1), Admit::Reject); // second probe blocked while inflight
        b.on_success();
        // need 2 successes to close
        assert_eq!(b.permitted_at(t1), Admit::Allow);
        b.on_success();
        assert!(!b.is_open());
        assert_eq!(b.permitted_at(t1), Admit::Allow); // closed
    }

    #[test]
    fn half_open_probe_failure_reopens() {
        let b = CircuitBreaker::new("pdp", cfg());
        let t0 = Instant::now();
        for _ in 0..3 { b.on_failure_at(t0); }
        let t1 = t0 + Duration::from_secs(11);
        assert_eq!(b.permitted_at(t1), Admit::Allow); // half-open probe
        b.on_failure_at(t1);                           // probe fails
        assert!(b.is_open());
        assert_eq!(b.permitted_at(t1), Admit::Reject);
    }
}
