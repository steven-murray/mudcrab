//! Install-time actions.
//!
//! Actions are a closed, typed set: `ModAction` is an internally-tagged serde
//! enum, so an unrecognised `action = "..."` is a *parse* error naming the
//! supported values, rather than something silently skipped at install time
//! with a log line the user never sees.
//!
//! The same type serves per-mod actions and install-wide actions (the latter
//! currently only those desugared from the top-level `[ini]` table), so there
//! is one dispatcher rather than two implementations that drifted apart.

pub mod create_dummy_plugin;
pub mod file_hide;
pub mod file_prune;
pub mod ini_set;
pub mod pack_bsa;
pub mod qac;

use crate::config::install::InstallSettings;
use crate::config::schema::ModAction;
use std::path::Path;

/// Everything an action needs to run.
pub struct ActionCx<'a> {
    /// Mod id, or "plan" for install-wide actions. Used in logs and errors.
    pub owner: &'a str,
    pub settings: &'a InstallSettings,
    /// The mod's staged data folder. `None` for install-wide actions, which is
    /// what makes `scope = "mod"` invalid there.
    pub mod_target: Option<&'a Path>,
}

pub fn apply_all(actions: &[ModAction], cx: &ActionCx<'_>) -> anyhow::Result<()> {
    for action in actions {
        apply_one(action, cx)
            .map_err(|err| err.context(format!("{}: action '{}'", cx.owner, action.name())))?;
    }
    Ok(())
}

fn apply_one(action: &ModAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    match action {
        ModAction::IniSet(spec) => ini_set::apply(spec, cx),
        ModAction::Qac(spec) => qac::apply(spec, cx),
        ModAction::PackBsa(spec) => pack_bsa::apply(spec, cx),
        ModAction::CreateDummyPlugin(spec) => create_dummy_plugin::apply(spec, cx),
        ModAction::FilePrune(spec) => file_prune::apply(spec, cx),
        ModAction::FileHide(spec) => file_hide::apply(spec, cx),
    }
}

#[cfg(test)]
mod tests {
    use crate::config::schema::{IniScope, IniSetFormat, ModAction};

    fn parse(toml_src: &str) -> Result<ModAction, toml::de::Error> {
        toml::from_str(toml_src)
    }

    #[test]
    fn unknown_action_is_a_parse_error_naming_the_supported_values() {
        let err = parse(r#"action = "definitely_not_real""#).unwrap_err();
        let msg = err.to_string();
        // Previously this warned at install time and silently skipped the action.
        for expected in [
            "ini_set",
            "qac",
            "pack_bsa",
            "create_dummy_plugin",
            "file_prune",
        ] {
            assert!(msg.contains(expected), "{expected} missing from: {msg}");
        }
    }

    #[test]
    fn unknown_field_in_a_known_action_is_a_parse_error() {
        let err = parse(
            r#"
            action = "ini_set"
            file = "a.ini"
            key = "k"
            value = "v"
            typoed_field = 1
        "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("typoed_field"), "{err}");
    }

    #[test]
    fn qac_without_plugins_is_a_parse_error() {
        let err = parse(r#"action = "qac""#).unwrap_err();
        assert!(err.to_string().contains("plugins"), "{err}");
    }

    #[test]
    fn ini_value_accepts_any_scalar_and_normalises_to_string() {
        let cases = [
            (r#"value = "0""#, "0"),
            (r#"value = 0"#, "0"),
            (r#"value = 2560"#, "2560"),
            (r#"value = 1.5"#, "1.5"),
            (r#"value = false"#, "0"),
            (r#"value = true"#, "1"),
        ];
        for (fragment, expected) in cases {
            let src = format!("action = \"ini_set\"\nfile=\"a.ini\"\nkey=\"k\"\n{fragment}");
            let ModAction::IniSet(spec) = parse(&src).unwrap() else {
                panic!("expected ini_set for {fragment}");
            };
            assert_eq!(spec.value.0, expected, "for {fragment}");
        }
    }

    #[test]
    fn ini_set_defaults_are_mod_scope_and_standard_format() {
        let ModAction::IniSet(spec) =
            parse("action=\"ini_set\"\nfile=\"a.ini\"\nkey=\"k\"\nvalue=\"v\"").unwrap()
        else {
            panic!("expected ini_set");
        };
        assert_eq!(spec.scope, IniScope::Mod);
        assert_eq!(spec.format, IniSetFormat::Standard);
    }

    #[test]
    fn ini_set_parses_game_scope_and_set_to_format() {
        let ModAction::IniSet(spec) = parse(
            r#"
            action = "ini_set"
            scope = "game"
            format = "set-to"
            file = "Ini/MigMiscellanea.ini"
            key = "zzzMigckQ.bBetterSkillup"
            value = "0"
        "#,
        )
        .unwrap() else {
            panic!("expected ini_set");
        };
        assert_eq!(spec.scope, IniScope::Game);
        assert_eq!(spec.format, IniSetFormat::SetTo);
    }

    #[test]
    fn pack_bsa_requires_an_output_and_defaults_to_packing_everything() {
        let err = parse(r#"action = "pack_bsa""#).unwrap_err();
        assert!(err.to_string().contains("output"), "{err}");

        let ModAction::PackBsa(spec) =
            parse("action=\"pack_bsa\"\noutput=\"Example.bsa\"").unwrap()
        else {
            panic!("expected pack_bsa");
        };
        assert_eq!(spec.output, "Example.bsa");
        assert!(spec.include.is_empty(), "no include means everything");
        assert!(spec.exclude.is_empty());
    }

    #[test]
    fn create_dummy_plugin_requires_an_output() {
        let err = parse(r#"action = "create_dummy_plugin""#).unwrap_err();
        assert!(err.to_string().contains("output"), "{err}");
    }

    #[test]
    fn file_prune_without_paths_is_a_parse_error() {
        // Matching the qac rule: a prune with nothing to delete is a mistake.
        let err = parse(r#"action = "file_prune""#).unwrap_err();
        assert!(err.to_string().contains("paths"), "{err}");
    }

    #[test]
    fn bsa_actions_round_trip_through_json() {
        // compiled.json / plan.json carry these between pipeline stages, and
        // the ordering they preserve is what file_prune depends on.
        let actions: Vec<ModAction> = vec![
            parse("action=\"pack_bsa\"\noutput=\"E.bsa\"\nexclude=[\"*.esp\"]").unwrap(),
            parse("action=\"create_dummy_plugin\"\noutput=\"E.esp\"").unwrap(),
            parse("action=\"file_prune\"\npaths=[\"meshes/**\"]").unwrap(),
        ];

        let json = serde_json::to_string(&actions).unwrap();
        let back: Vec<ModAction> = serde_json::from_str(&json).unwrap();

        assert_eq!(
            back.iter().map(ModAction::name).collect::<Vec<_>>(),
            vec!["pack_bsa", "create_dummy_plugin", "file_prune"],
            "declaration order must survive the pipeline"
        );
        let ModAction::PackBsa(spec) = &back[0] else {
            panic!("expected pack_bsa");
        };
        assert_eq!(spec.exclude, vec!["*.esp".to_string()]);
    }

    #[test]
    fn actions_round_trip_through_json() {
        // compiled.json / plan.json carry these between pipeline stages.
        let src = "action=\"ini_set\"\nfile=\"a.ini\"\nkey=\"k\"\nvalue=42\nscope=\"game\"";
        let parsed = parse(src).unwrap();
        let json = serde_json::to_string(&parsed).unwrap();
        let back: ModAction = serde_json::from_str(&json).unwrap();
        let (ModAction::IniSet(a), ModAction::IniSet(b)) = (&parsed, &back) else {
            panic!("expected ini_set");
        };
        assert_eq!(a.value, b.value);
        assert_eq!(a.scope, b.scope);
    }
}
