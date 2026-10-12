pub mod discover;
pub mod loot;
pub mod xedit;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

static PATH_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Machine-local tool configuration.  Loaded from a `tools.toml` file whose
/// path is passed on the command line or defaults to
/// `~/.config/mudcrab/tools.toml`.  Missing files are treated as an empty
/// config (all defaults apply).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ToolsConfig {
    /// The downloads folder every command uses unless `--downloads` says
    /// otherwise. One central folder, with each MO2 instance pointed at it, is
    /// the recommended setup.
    pub downloads_dir: Option<PathBuf>,
    /// The Nexus API key, from https://next.nexusmods.com/settings/api-keys.
    /// `NEXUS_API_KEY` in the environment takes precedence.
    pub nexus_api_key: Option<String>,
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

        if let Some(path) = sibling_qac
            && path.exists()
        {
            return path;
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

    /// The Nexus API key: `NEXUS_API_KEY` if set, else `nexus_api_key` in the
    /// default tools.toml. A tools.toml that cannot be read gives no key; the
    /// commands that need one say how to set it.
    pub fn nexus_api_key() -> Option<String> {
        let from_env = std::env::var("NEXUS_API_KEY")
            .ok()
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty());
        from_env.or_else(|| {
            let config = Self::load(&Self::default_path()?).ok()?;
            config
                .nexus_api_key
                .map(|key| key.trim().to_string())
                .filter(|key| !key.is_empty())
        })
    }

    /// The tools.toml every command uses: `--tools-config`, else
    /// `MUDCRAB_TOOLS_CONFIG`, else the standard place. The first time it is
    /// not the standard place, says so, so a forgotten variable is visible.
    pub fn default_path() -> Option<PathBuf> {
        match Self::chosen_path() {
            Some((path, from)) => {
                static TOLD: std::sync::Once = std::sync::Once::new();
                TOLD.call_once(|| eprintln!("note: using {} ({from})", path.display()));
                Some(path)
            }
            None => Self::standard_path(),
        }
    }

    /// The tools.toml named by `--tools-config` or `MUDCRAB_TOOLS_CONFIG`, and
    /// which one named it; `None` for the standard place.
    pub fn chosen_path() -> Option<(PathBuf, &'static str)> {
        match PATH_OVERRIDE.get() {
            Some(path) => Some((path.clone(), "--tools-config")),
            None => std::env::var_os("MUDCRAB_TOOLS_CONFIG")
                .filter(|value| !value.is_empty())
                .map(|value| (PathBuf::from(value), "MUDCRAB_TOOLS_CONFIG")),
        }
    }

    /// Point every command at this tools.toml (the global `--tools-config`).
    /// Only the first call counts.
    pub fn set_path_override(path: PathBuf) {
        let _ = PATH_OVERRIDE.set(path);
    }

    /// Where tools.toml lives when nothing says otherwise:
    /// `~/.config/mudcrab/tools.toml`.
    pub fn standard_path() -> Option<PathBuf> {
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
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
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
                if let Some(pfx_name) = wine.prefix.file_name().and_then(|n| n.to_str())
                    && pfx_name.eq_ignore_ascii_case("pfx")
                    && let Some(compat_data_path) = wine.prefix.parent()
                {
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

                // Last-resort fallback for Proton wrappers if the path parser above
                // could not discover a Steam root from prefix structure.
                if !set_client_install_path && let Some(home) = std::env::var_os("HOME") {
                    let default_steam_root = PathBuf::from(home).join(".local/share/Steam");
                    cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", default_steam_root);
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

/// Values chosen for `tools.toml`; each `None` is written as a placeholder.
#[derive(Debug, Default, Clone)]
pub struct ToolValues {
    pub downloads_dir: Option<PathBuf>,
    pub nexus_api_key: Option<String>,
    pub wine_prefix: Option<PathBuf>,
    pub proton: Option<PathBuf>,
    pub tes4edit: Option<PathBuf>,
    pub loot_appdata: Option<PathBuf>,
}

fn toml_path(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}

/// Generate `tools.toml` for the detected tool set: the chosen values where
/// there are some, clearly-marked placeholders where not. Entries for tools
/// the list does not need are left out.
pub fn generate_tools_toml(required: &RequiredTools, values: &ToolValues) -> String {
    let mut out = String::new();

    out.push_str("# mudcrab tools.toml — machine-local tool configuration\n");
    out.push_str("# This file should NOT be committed to version control.\n");
    out.push_str("# Generated by: mudcrab setup-tools\n\n");

    // Top level, so before any table. Every list wants one.
    out.push_str("# The downloads folder every command uses unless given --downloads:\n");
    out.push_str("# one folder, shared by every list and every MO2 instance.\n");
    match &values.downloads_dir {
        Some(dir) => out.push_str(&format!("downloads_dir = {}\n\n", toml_path(dir))),
        #[cfg(not(target_os = "windows"))]
        None => out.push_str("# downloads_dir = \"/home/you/Games/OblivionModDownloads\"\n\n"),
        #[cfg(target_os = "windows")]
        None => out.push_str("# downloads_dir = \"D:\\\\Games\\\\OblivionModDownloads\"\n\n"),
    }

    out.push_str("# Your Nexus API key (https://next.nexusmods.com/settings/api-keys).\n");
    out.push_str("# NEXUS_API_KEY in the environment overrides it.\n");
    match &values.nexus_api_key {
        Some(key) => out.push_str(&format!(
            "nexus_api_key = \"{}\"\n\n",
            key.replace('\\', "\\\\").replace('"', "\\\"")
        )),
        None => out.push_str("# nexus_api_key = \"...\"\n\n"),
    }

    // Wine section — only on Linux and only when at least one Windows tool is needed.
    // On Windows this section is not needed and is omitted.
    #[cfg(not(target_os = "windows"))]
    if required.needs_wine_on_linux() {
        out.push_str("# Shared Wine / Proton prefix for all Windows-only tools.\n");
        out.push_str("[wine]\n");
        out.push_str("# Path to the Wine prefix directory (the folder containing drive_c/).\n");
        match &values.wine_prefix {
            Some(prefix) => out.push_str(&format!("prefix = {}\n", toml_path(prefix))),
            None => out.push_str("prefix = \"/path/to/your/wine-prefix\"\n"),
        }
        out.push_str("# Optional: use a specific Proton build instead of system wine.\n");
        match &values.proton {
            Some(proton) => out.push_str(&format!("proton = {}\n\n", toml_path(proton))),
            None => out.push_str(
                "# proton = \"/home/user/.steam/root/steamapps/common/Proton 9.0/proton\"\n\n",
            ),
        }
    }

    if required.needs_loot {
        out.push_str("# LOOT is a native binary on Linux; no Wine wrapper needed.\n");
        out.push_str("[loot]\n");
        out.push_str("# Optional: override if LOOT is not on your PATH.\n");
        out.push_str("# exe = \"/usr/local/bin/LOOT\"\n");
        if let Some(appdata) = &values.loot_appdata {
            out.push_str(&format!("game_appdata_path = {}\n", toml_path(appdata)));
        }
        out.push('\n');
    }

    if required.needs_tes4edit {
        out.push_str("# TES4Edit / xEdit — run through Wine on Linux. Its Quick Auto Clean\n");
        out.push_str("# executable is found beside it.\n");
        out.push_str("[tes4edit]\n");
        match &values.tes4edit {
            Some(exe) => out.push_str(&format!("exe = {}\n\n", toml_path(exe))),
            #[cfg(not(target_os = "windows"))]
            None => out
                .push_str("exe = \"/path/to/your/wine-prefix/drive_c/tools/xEdit/xEdit.exe\"\n\n"),
            #[cfg(target_os = "windows")]
            None => out.push_str("exe = \"C:\\\\tools\\\\xEdit\\\\xEdit.exe\"\n\n"),
        }
    }

    if out.ends_with("\n\n") {
        out.truncate(out.len() - 1);
    }

    out
}
