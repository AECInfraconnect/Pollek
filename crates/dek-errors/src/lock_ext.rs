// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! lock_ext.rs — poison-safe Mutex/RwLock access (Phase B reliability).
//!
//! `Mutex::lock().unwrap()` aborts the whole process on poisoning because the
//! workspace is built with `panic = "abort"`. For a long-running enforcement
//! daemon that is the wrong failure mode: a poisoned lock from one unrelated
//! panic should not take down policy enforcement. These extensions recover the
//! guard instead of aborting.
//!
//! Usage:
//! ```ignore
//! use dek_errors::lock_ext::LockExt;
//! let guard = self.conn.lock_safe();      // never panics on poison
//! let guard = self.stats.read_safe();     // RwLock read
//! let mut g = self.state.write_safe();    // RwLock write
//! ```

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait LockExt<T> {
    /// Lock, recovering the guard if the mutex was poisoned (logs once).
    fn lock_safe(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!("recovered from poisoned Mutex (continuing fail-safe)");
                poisoned.into_inner()
            }
        }
    }
}

pub trait RwLockExt<T> {
    fn read_safe(&self) -> RwLockReadGuard<'_, T>;
    fn write_safe(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn read_safe(&self) -> RwLockReadGuard<'_, T> {
        match self.read() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!("recovered from poisoned RwLock (read)");
                poisoned.into_inner()
            }
        }
    }
    fn write_safe(&self) -> RwLockWriteGuard<'_, T> {
        match self.write() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!("recovered from poisoned RwLock (write)");
                poisoned.into_inner()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::sync::Arc;

    #[test]
    fn recovers_poisoned_mutex() {
        let m = Arc::new(Mutex::new(41));
        let m2 = m.clone();
        // poison it
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = m2.lock().unwrap();
            panic!("boom");
        }));
        // lock_safe still works (would abort with .lock().unwrap())
        let mut g = m.lock_safe();
        *g += 1;
        assert_eq!(*g, 42);
    }
}
