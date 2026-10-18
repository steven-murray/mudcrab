//! Running a program on the host when mudcrab itself runs in a Flatpak.
//!
//! The Flatpak's sandbox has its own processes and none of the host's tools:
//! Wine and Proton are the host's, and so is any Mod Organizer 2 that is
//! open. A program that belongs to the host is started through
//! `flatpak-spawn --host`, which needs the sandbox to be allowed to talk to
//! `org.freedesktop.Flatpak` (the manifest grants it). Outside a Flatpak all
//! of this is a no-op.
//!
//! `flatpak-spawn` does not pass a signal on to the host program's children:
//! killing it leaves Proton's real xEdit window running. So a program that
//! may have to be closed is started in a process group of its own on the
//! host, with the group's id written to a file, and closed by signalling that
//! group there.

use std::path::Path;
use std::process::Command;

/// Whether mudcrab is running inside a Flatpak sandbox.
pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// `command` as it should be started: unchanged outside a Flatpak; inside
/// one, through `flatpak-spawn --host`, with its environment and working
/// directory carried across (`flatpak-spawn` passes neither by itself).
/// Stdio and process-group settings are not carried: set them on the result.
pub fn on_host(command: Command) -> Command {
    wrap(command, None)
}

/// As [`on_host`], but on the host in a process group of its own whose id is
/// written to `group_file`, for [`close_host_group`].
pub fn on_host_in_group(command: Command, group_file: &Path) -> Command {
    wrap(command, Some(group_file))
}

fn wrap(command: Command, group_file: Option<&Path>) -> Command {
    if !in_flatpak() {
        return command;
    }
    let mut host = Command::new("flatpak-spawn");
    host.arg("--host").arg("--watch-bus");
    if let Some(dir) = command.get_current_dir() {
        host.arg(format!("--directory={}", dir.display()));
    }
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            host.arg(format!(
                "--env={}={}",
                key.to_string_lossy(),
                value.to_string_lossy()
            ));
        }
    }
    if let Some(file) = group_file {
        // `setsid` makes the shell lead a new group; it writes its pid (the
        // group's id) and becomes the program.
        host.args(["setsid", "sh", "-c", r#"echo $$ > "$0"; exec "$@""#])
            .arg(file);
    }
    host.arg(command.get_program()).args(command.get_args());
    host
}

/// Signal the host process group whose id `group_file` holds (from
/// [`on_host_in_group`]), as `kill -<signal> -- -<group>`. Best effort.
pub fn close_host_group(group_file: &Path, signal: &str) {
    let Some(group) = std::fs::read_to_string(group_file)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
    else {
        return;
    };
    let _ = Command::new("flatpak-spawn")
        .args(["--host", "kill", &format!("-{signal}"), "--"])
        .arg(format!("-{group}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    #[test]
    fn outside_a_flatpak_a_command_is_left_alone() {
        if super::in_flatpak() {
            return;
        }
        let mut command = std::process::Command::new("wine");
        command.arg("x.exe").env("WINEPREFIX", "/p");
        let out = super::on_host(command);
        assert_eq!(out.get_program(), "wine");
        assert_eq!(out.get_args().collect::<Vec<_>>(), ["x.exe"]);
    }
}
