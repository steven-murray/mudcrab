//! Starting the console programs mudcrab runs for itself -- 7-Zip, `tar`,
//! `git` -- without a window.
//!
//! On Windows a program with a window of its own (the GUI) that starts a
//! console program gets a new console window for it, opened and closed
//! around every run: one flash per archive unpacked. `CREATE_NO_WINDOW`
//! starts it with none. Elsewhere, and for the tools that have windows of
//! their own (xEdit), nothing changes.

use std::ffi::OsStr;
use std::process::Command;

/// `program`, to be started with no console window of its own.
pub fn console(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
