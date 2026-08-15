use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Machine-local tool configuration.  Loaded from a `tools.toml` file whose
/// path is passed on the command line or defaults to
/// `~/.config/mudcrab/tools.toml`.  Missing files are treated as an empty
/// config (all defaults apply).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ToolsConfig {
    /// Wine / Proton runner used on non-Windows hosts for Windows-only tools.
    /// On Windows this section is ignored.
    pub wine: Option<WineConfig>,
    /// Settings for LOOT.  Optional; defaults to `LOOT` on PATH.
    pub loot: Option<LootConfig>,
    /// Settings for TES4Edit / xEdit.  Required if any mod declares a `qac` action.
    pub tes4edit: Option<Tes4EditConfig>,
}

/// Wine or Proton prefix shared by all Windows-only tools.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WineConfig {
    /// Path to the Wine prefix directory (the folder that contains `drive_c/`).
    pub prefix: PathBuf,
    /// Optional: path to a `proton` executable.  When set, tools are invoked
    /// as `proton run <exe>` instead of `wine <exe>`.
    pub proton: Option<PathBuf>,
}

/// LOOT tool settings.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LootConfig {
    /// Path to the LOOT executable.  Defaults to `"LOOT"` (assumed on PATH).
    /// On Linux, LOOT is a native binary and does NOT go through Wine.
    pub exe: Option<String>,
    /// Path to the game's local AppData directory where LOOT reads/writes its
    /// plugin and load-order files (eg `plugins.txt` / `loadorder.txt`).
    /// On Linux with a Steam/Proton installation this is
    /// typically the Wine-prefix path, e.g.:
    ///   `~/.local/share/Steam/steamapps/compatdata/22330/pfx/drive_c/users/steamuser/AppData/Local/Oblivion`
    /// When set, mudcrab writes the desired plugin list there before invoking
    /// LOOT `--auto-sort`, lets LOOT auto-detect the game, and reads the
    /// sorted result back from the same directory.  When unset, mudcrab falls
    /// back to a temporary
    /// sandbox that may not work if LOOT overrides the path via Steam detection.
    pub game_appdata_path: Option<PathBuf>,
}

/// TES4Edit / xEdit tool settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tes4EditConfig {
    /// Path to the xEdit executable (e.g. `TES4Edit.exe`).
    /// On Linux this is run through the configured `[wine]` prefix.
    pub exe: PathBuf,
    /// Optional path to the dedicated Quick Auto Clean executable
    /// (e.g. `TES4EditQuickAutoClean.exe`). When omitted, mudcrab will prefer
    /// a sibling `TES4EditQuickAutoClean.exe` next to `exe` if one exists,
    /// otherwise it falls back to `exe`.
    pub qac_exe: Option<PathBuf>,
}

impl Tes4EditConfig {
    pub fn qac_executable(&self) -> PathBuf {
        if let Some(qac_exe) = self.qac_exe.as_deref() {
            return qac_exe.to_path_buf();
        }

        let sibling_qac = self
            .exe
            .parent()
            .map(|parent| parent.join("TES4EditQuickAutoClean.exe"));

        if let Some(path) = sibling_qac {
            if path.exists() {
                return path;
            }
        }

        self.exe.clone()
    }
}

impl ToolsConfig {
    /// Load from a TOML file.  Returns `Default::default()` if the file does
    /// not exist, so callers don't need to special-case a missing tools.toml.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
        toml::from_str(&raw)
            .map_err(|err| anyhow::anyhow!("failed to parse {}: {err}", path.display()))
    }

    /// Return the default tools.toml path: `~/.config/mudcrab/tools.toml`.
    pub fn default_path() -> Option<PathBuf> {
        // Use $XDG_CONFIG_HOME if set, otherwise fall back to $HOME/.config on
        // non-Windows, and %APPDATA% on Windows.
        #[cfg(target_os = "windows")]
        {
            std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("mudcrab").join("tools.toml"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|h| PathBuf::from(h).join(".config"))
                })
                .map(|cfg| cfg.join("mudcrab").join("tools.toml"))
        }
    }

    /// Build a `Command` for invoking LOOT.
    /// LOOT is always a native binary (no Wine needed on Linux).
    pub fn loot_command(&self) -> Command {
        let exe = self
            .loot
            .as_ref()
            .and_then(|l| l.exe.as_deref())
            .unwrap_or("LOOT");
        Command::new(exe)
    }

    /// Build a `Command` for a Windows-only tool (e.g. TES4Edit).
    ///
    /// * On **Windows** the executable is invoked directly.
    /// * On **non-Windows** the executable is wrapped in `wine` (or `proton run`)
    ///   using the configured `[wine]` prefix.  Returns an error if `[wine]` is
    ///   not configured.
    pub fn windows_tool_command(&self, exe: &Path) -> anyhow::Result<Command> {
        #[cfg(target_os = "windows")]
        {
            Ok(Command::new(exe))
        }

        #[cfg(not(target_os = "windows"))]
        {
            let wine = self.wine.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "running Windows-only tools on Linux requires a [wine] section in tools.toml \
                     (set 'prefix' to your Wine prefix directory, or 'proton' to a Proton executable)"
                )
            })?;

            let using_proton = wine.proton.is_some();
            let mut cmd = if let Some(proton) = &wine.proton {
                let mut c = Command::new(proton);
                c.arg("run").arg(exe);
                c
            } else {
                let mut c = Command::new("wine");
                c.arg(exe);
                c
            };

            cmd.env("WINEPREFIX", &wine.prefix);

            // Proton wrappers commonly require STEAM_COMPAT_DATA_PATH to point
            // at the compatdata root (the parent of "pfx"). Derive it from
            // the configured prefix when possible.
            if using_proton {
                let mut set_client_install_path = false;
                // Derive STEAM_COMPAT_DATA_PATH and STEAM_COMPAT_CLIENT_INSTALL_PATH
                // from <steam_root>/steamapps/compatdata/<appid>/pfx.
                if let Some(pfx_name) = wine.prefix.file_name().and_then(|n| n.to_str()) {
                    if pfx_name.eq_ignore_ascii_case("pfx") {
                        if let Some(compat_data_path) = wine.prefix.parent() {
                            cmd.env("STEAM_COMPAT_DATA_PATH", compat_data_path);

                            let mut current = compat_data_path.parent();
                            while let Some(path) = current {
                                if path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|n| n.eq_ignore_ascii_case("steamapps"))
                                    .unwrap_or(false)
                                {
                                    if let Some(steam_root) = path.parent() {
                                        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", steam_root);
                                        set_client_install_path = true;
                                    }
                                    break;
                                }
                                current = path.parent();
                            }
                        }
                    }
                }

                // Last-resort fallback for Proton wrappers if the path parser above
                // could not discover a Steam root from prefix structure.
                if !set_client_install_path {
                    if let Some(home) = std::env::var_os("HOME") {
                        let default_steam_root = PathBuf::from(home).join(".local/share/Steam");
                        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", default_steam_root);
                    }
                }
            }

            Ok(cmd)
        }
    }

    /// Convert a Unix filesystem path to the equivalent Windows path inside a
    /// Wine prefix.  Wine maps the Unix root `/` to drive letter `Z:`, so
    /// `/tmp/foo/bar` becomes `Z:\tmp\foo\bar`.
    ///
    /// Used to pass Unix-side paths as `-D:<path>` arguments to Wine-hosted
    /// Windows executables.
    #[cfg(not(target_os = "windows"))]
    pub fn unix_path_to_wine(path: &Path) -> String {
        let unix = path.to_string_lossy();
        let without_leading_slash = unix.trim_start_matches('/');
        let windows_sep = without_leading_slash.replace('/', "\\");
        format!("Z:\\{windows_sep}")
    }
}

/// Which tools are required by a given source modlist.
/// Used by the `setup-tools` command to generate a targeted template.
#[derive(Debug, Default)]
pub struct RequiredTools {
    pub needs_loot: bool,
    pub needs_tes4edit: bool,
}

impl RequiredTools {
    /// Returns `true` if any Windows-only tool that needs Wine was detected.
    pub fn needs_wine_on_linux(&self) -> bool {
        self.needs_tes4edit
    }
}

/// Generate a `tools.toml` template string tailored to the detected tool set.
/// Entries for unneeded tools are omitted.  All values that require user input
/// are left as clearly-marked placeholders.
pub fn generate_tools_toml(required: &RequiredTools) -> String {
    let mut out = String::new();

    out.push_str("# mudcrab tools.toml — machine-local tool configuration\n");
    out.push_str("# This file should NOT be committed to version control.\n");
    out.push_str("# Generated by: mudcrab setup-tools\n\n");

    // Wine section — only on Linux and only when at least one Windows tool is needed.
    // On Windows this section is not needed and is omitted.
    #[cfg(not(target_os = "windows"))]
    if required.needs_wine_on_linux() {
        out.push_str("# Shared Wine / Proton prefix for all Windows-only tools.\n");
        out.push_str("[wine]\n");
        out.push_str("# Path to the Wine prefix directory (the folder containing drive_c/).\n");
        out.push_str("prefix = \"/path/to/your/wine-prefix\"\n");
        out.push_str("# Optional: use a specific Proton build instead of system wine.\n");
        out.push_str("# proton = \"/home/user/.steam/root/steamapps/common/Proton 9.0/proton\"\n\n");
    }

    if required.needs_loot {
        out.push_str("# LOOT is a native binary on Linux; no Wine wrapper needed.\n");
        out.push_str("[loot]\n");
        out.push_str("# Optional: override if LOOT is not on your PATH.\n");
        out.push_str("# exe = \"/usr/local/bin/LOOT\"\n\n");
    }

    if required.needs_tes4edit {
        out.push_str("# TES4Edit / xEdit — run through Wine on Linux.\n");
        out.push_str("[tes4edit]\n");
        out.push_str("# Path to the xEdit .exe inside your Wine prefix.\n");

        #[cfg(not(target_os = "windows"))]
        out.push_str("exe = \"/path/to/your/wine-prefix/drive_c/tools/xEdit/xEdit.exe\"\n\n");

        #[cfg(target_os = "windows")]
        out.push_str("exe = \"C:\\\\tools\\\\xEdit\\\\xEdit.exe\"\n\n");
    }

    if out.ends_with("\n\n") {
        out.truncate(out.len() - 1);
    }

    out
}
