use crate::cli::SetupToolsArgs;
use crate::config::loader::load_modlist;
use crate::config::schema::ModAction;
use crate::config::schema::PostInstallAction;
use crate::config::tools::discover::{self, Candidate};
use crate::config::tools::{RequiredTools, ToolValues, ToolsConfig, generate_tools_toml};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// Ask for one path, offering what was found. Enter takes the first, a number
/// picks another, `-` leaves it as a placeholder, anything else is a path.
/// Not interactive: the first found, if any.
fn pick(
    question: &str,
    found: &[Candidate],
    interactive: bool,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> anyhow::Result<Option<PathBuf>> {
    if !interactive {
        return Ok(found.first().map(|c| c.path.clone()));
    }
    writeln!(output, "\n{question}")?;
    if found.is_empty() {
        writeln!(
            output,
            "  (nothing found; type a path, or - to fill it in later)"
        )?;
    }
    for (index, candidate) in found.iter().enumerate() {
        writeln!(output, "  {}. {}", index + 1, candidate.path.display())?;
        writeln!(output, "       {}", candidate.why)?;
    }
    loop {
        let hint = if found.is_empty() { "path or -" } else { "1" };
        write!(output, "  Your choice [{hint}]: ")?;
        output.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            anyhow::bail!("input ended before \"{question}\" was answered");
        }
        let answer = line.trim();
        if answer == "-" {
            return Ok(None);
        }
        if answer.is_empty() {
            match found.first() {
                Some(first) => return Ok(Some(first.path.clone())),
                None => continue,
            }
        }
        if let Ok(number) = answer.parse::<usize>()
            && let Some(candidate) = number.checked_sub(1).and_then(|i| found.get(i))
        {
            return Ok(Some(candidate.path.clone()));
        }
        let path = PathBuf::from(shellexpand_home(answer));
        if path.exists() {
            return Ok(Some(path));
        }
        writeln!(
            output,
            "  {} does not exist; type another, or - to skip",
            path.display()
        )?;
    }
}

/// `~/x` -> `$HOME/x`, as a shell would.
fn shellexpand_home(text: &str) -> String {
    match (text.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => text.to_string(),
    }
}

/// Where `tools.toml` goes, and why there, in words.
fn explain_location(path: &Path, explicit: bool) -> String {
    if explicit {
        return format!("Writing {} (--output).", path.display());
    }
    if cfg!(windows) {
        return format!("Writing {} (%APPDATA%\\mudcrab).", path.display());
    }
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) => format!(
            "Writing {} (XDG_CONFIG_HOME is {}).",
            path.display(),
            PathBuf::from(dir).display()
        ),
        None => format!(
            "Writing {} (XDG_CONFIG_HOME is not set in mudcrab's environment, so ~/.config; \
             in fish, `set -x` exports it).",
            path.display()
        ),
    }
}

pub async fn run(args: SetupToolsArgs) -> anyhow::Result<()> {
    let source = load_modlist(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to parse {}: {err}", args.input.display()))?;

    // Detect which tools are referenced anywhere in the modlist.
    let mut required = RequiredTools::default();

    if source
        .post_install_actions
        .iter()
        .any(|a| matches!(a, PostInstallAction::LootSort))
    {
        required.needs_loot = true;
    }

    let flat = source.flatten_mods().map_err(|err| {
        anyhow::anyhow!("failed to flatten mods while scanning for required tools: {err}")
    })?;
    for (_id, spec) in &flat {
        for action in &spec.actions {
            if matches!(action.action, ModAction::Qac(_)) {
                required.needs_tes4edit = true;
            }
        }
    }

    // Report what was found.
    tracing::info!(
        needs_loot = required.needs_loot,
        needs_tes4edit = required.needs_tes4edit,
        "setup-tools: tool scan complete"
    );

    if !required.needs_loot && !required.needs_tes4edit {
        tracing::info!("setup-tools: no external tools required by this modlist");
    }

    let output_path = args
        .output
        .clone()
        // Where the other commands look when not told, so the template
        // takes effect once filled in.
        .or_else(ToolsConfig::default_path)
        .unwrap_or_else(|| PathBuf::from("tools.toml"));

    if output_path.exists() && !args.force {
        anyhow::bail!(
            "{} already exists; pass --force to overwrite",
            output_path.display()
        );
    }
    println!("{}", explain_location(&output_path, args.output.is_some()));

    // Each value the list needs, from what is found where such things usually
    // live; asked for in a terminal, the best guess otherwise.
    let interactive = !args.defaults && std::io::stdin().is_terminal();
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut out = std::io::stdout();
    let roots = discover::search_roots();
    let mut values = ToolValues {
        downloads_dir: pick(
            "Downloads folder: where archives are kept (one, shared by every list and MO2 instance).",
            &discover::downloads_dirs(&roots, &discover::shortcuts()),
            interactive,
            &mut input,
            &mut out,
        )?,
        ..ToolValues::default()
    };
    if required.needs_wine_on_linux() && !cfg!(windows) {
        values.wine_prefix = pick(
            "Wine/Proton prefix to run xEdit in (the folder holding drive_c). MO2's own prefix is a good choice.",
            &discover::wine_prefixes(),
            interactive,
            &mut input,
            &mut out,
        )?;
        values.proton = pick(
            "Proton to run it with (- for the system's wine).",
            &discover::proton_builds(values.wine_prefix.as_deref()),
            interactive,
            &mut input,
            &mut out,
        )?;
    }
    if required.needs_tes4edit {
        values.tes4edit = pick(
            "xEdit (TES4Edit.exe or xEdit.exe), for cleaning plugins.",
            &discover::xedit_exes(&roots, values.wine_prefix.as_deref()),
            interactive,
            &mut input,
            &mut out,
        )?;
    }
    if required.needs_loot {
        values.loot_appdata = discover::wine_prefixes()
            .iter()
            .map(|c| {
                c.path
                    .join("drive_c/users/steamuser/AppData/Local/Oblivion")
            })
            .find(|p| p.is_dir());
    }
    let content = generate_tools_toml(&required, &values);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            anyhow::anyhow!(
                "failed to create output directory {}: {err}",
                parent.display()
            )
        })?;
    }

    std::fs::write(&output_path, &content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", output_path.display()))?;

    tracing::info!(path = %output_path.display(), "setup-tools: wrote tools.toml template");

    println!("\nWrote {}.", output_path.display());
    if content.contains("/path/to/") || content.contains("C:\\\\tools") {
        println!("Some values were left as placeholders; edit the file to fill them in.");
    }

    Ok(())
}
