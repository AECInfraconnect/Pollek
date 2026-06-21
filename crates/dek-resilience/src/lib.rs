//! dek-resilience — SaaS-scale fail-closed primitives for the PEP.
pub mod breaker;
pub mod admission;

pub use admission::{AdmissionControl, AdmitPermit};
pub use breaker::{Admit, CircuitBreaker, CircuitConfig};
