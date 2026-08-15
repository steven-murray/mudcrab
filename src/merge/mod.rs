//! Headless replacement for zEdit's zMerge.
//!
//! Combines several plugins into one, renumbering FormIDs and rewriting every
//! reference, so a modlist can be installed without driving a GUI tool.
//!
//! Design notes and the evidence behind them: `MOFAM-test/notes/merge-recon.md`.

pub mod alloc;
