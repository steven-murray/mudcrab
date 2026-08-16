//! Headless replacement for zEdit's zMerge.
//!
//! Combines several plugins into one, renumbering FormIDs and rewriting every
//! reference, so a modlist can be installed without driving a GUI tool.
//!
//! Design notes and the evidence behind them: `MOFAM-test/notes/merge-recon.md`.

pub mod alloc;
pub mod assemble;
pub mod audit;
pub mod masters;
pub mod rewrite;
pub mod run;

pub use audit::AuditError;
pub use run::{run, MergeError, MergeOutput, MergeReport, MergeRequest, MergeSource};
