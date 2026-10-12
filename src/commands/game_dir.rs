//! `game-dir`: where Oblivion is, in the form MO2's first-run setup wants.

use crate::cli::GameDirArgs;
use crate::config::tools::discover;
use std::path::Path;

/// A folder as a Windows program running under Wine or Proton sees it: the
/// Linux root is drive `Z:`. On Windows, the path itself.
fn as_mo2_sees_it(path: &Path) -> String {
    if cfg!(windows) {
        path.display().to_string()
    } else {
        format!("Z:{}", path.to_string_lossy().replace('/', "\\"))
    }
}

pub fn run(args: GameDirArgs) -> anyhow::Result<()> {
    let found = discover::game_dirs(&discover::steam_install_roots(), &discover::search_roots());
    let Some(first) = found.first() else {
        anyhow::bail!(
            "no Oblivion.exe found in a Steam library or under ~/Games. Install the game \
             and start it once from its launcher; if it is somewhere else, that folder is \
             the one to give MO2"
        );
    };
    if args.plain {
        println!("{}", as_mo2_sees_it(&first.path));
        return Ok(());
    }
    for (index, candidate) in found.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{} ({})", candidate.path.display(), candidate.why);
        if !cfg!(windows) {
            println!(
                "  in MO2's setup (under Proton): {}",
                as_mo2_sees_it(&candidate.path)
            );
        }
    }
    if found.len() > 1 {
        println!("\nMore than one copy: give MO2 the one you play.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn a_linux_folder_is_on_drive_z() {
        assert_eq!(
            as_mo2_sees_it(Path::new(
                "/home/me/.local/share/Steam/steamapps/common/Oblivion"
            )),
            "Z:\\home\\me\\.local\\share\\Steam\\steamapps\\common\\Oblivion"
        );
    }
}
