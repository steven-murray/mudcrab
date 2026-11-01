//! Stopping a long run part way: a download, a check, an install.
//!
//! A front end asks with [`request`]; the run looks with [`check`] before
//! each archive or mod and stops there, cleanly, with [`Cancelled`] as its
//! error. Nothing is stopped in the middle of an archive or an xEdit run:
//! what is half done would be worse than finishing it. An install records
//! each mod as it finishes, so one cancelled and run again carries on where
//! it stopped.

use std::sync::atomic::{AtomicBool, Ordering};

static REQUESTED: AtomicBool = AtomicBool::new(false);

/// The error a cancelled run ends with.
#[derive(Debug, thiserror::Error)]
#[error("cancelled")]
pub struct Cancelled;

/// Ask the run in progress to stop at its next archive or mod.
pub fn request() {
    REQUESTED.store(true, Ordering::Relaxed);
}

/// Forget a request: for a new run to start with.
pub fn reset() {
    REQUESTED.store(false, Ordering::Relaxed);
}

/// Whether a stop was asked for.
pub fn requested() -> bool {
    REQUESTED.load(Ordering::Relaxed)
}

/// `Err(Cancelled)` once a stop was asked for.
pub fn check() -> Result<(), Cancelled> {
    if requested() { Err(Cancelled) } else { Ok(()) }
}
