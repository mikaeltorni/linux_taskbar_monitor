//! Process-wide lock serializing every test in this crate that mutates
//! `PATH`/`HOME` (or reads them while another thread might mutate them).
//!
//! `std::env::set_var`/`remove_var` change global, per-process state that
//! `std::env::var`/`var_os` read with no synchronization of their own. A
//! private `static ENV_LOCK` scoped to a single module's `tests` submodule
//! only serializes *that module's* tests against each other — it does
//! nothing to stop a test in a *different* module from mutating the same
//! variables concurrently on another `cargo test` worker thread, which is
//! exactly the kind of concurrent-getenv/setenv hazard that made
//! `std::env::set_var` `unsafe` in modern Rust. Every test anywhere in this
//! crate that calls `std::env::set_var`, `std::env::remove_var`, or reads
//! `PATH`/`HOME` around such a mutation must hold [`lock`] for the whole
//! mutate-use-restore span, not a module-private mutex.

#![cfg(test)]

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the shared environment-mutation lock for the crate's test suite.
///
/// Recovers from a poisoned mutex (a panic in an earlier guarded test must
/// not permanently deadlock every later test that also needs this lock);
/// the inner `()` carries no state to invalidate, so continuing after
/// poisoning is safe.
pub fn lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
