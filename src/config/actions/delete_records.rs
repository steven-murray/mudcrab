//! `delete_records`: remove records or groups from a staged plugin.

use super::ActionCx;
use crate::config::schema::DeleteRecordsAction;
use crate::plugin::formid::FormId;
use crate::plugin::prune::PruneRequest;
use crate::plugin::Plugin;

pub(super) fn apply(action: &DeleteRecordsAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: delete_records is only valid as a per-mod action", cx.owner);
    };

    if action.groups.is_empty() && action.form_ids.is_empty() {
        anyhow::bail!(
            "{}: delete_records names neither groups nor form_ids, so it would do nothing",
            cx.owner
        );
    }

    let path = mod_target.join(&action.plugin);
    if !path.is_file() {
        anyhow::bail!(
            "{}: delete_records target '{}' is not in {}",
            cx.owner,
            action.plugin,
            mod_target.display()
        );
    }

    if cx.settings.dry_run {
        tracing::info!(
            owner = cx.owner,
            plugin = %action.plugin,
            groups = ?action.groups,
            form_ids = ?action.form_ids,
            "install dry-run delete_records action"
        );
        return Ok(());
    }

    let mut plugin = Plugin::read(&path)
        .map_err(|err| anyhow::anyhow!("{}: failed to read {}: {err}", cx.owner, path.display()))?;

    let own_index = plugin.masters.own_mod_index();
    let request = PruneRequest {
        groups: action
            .groups
            .iter()
            .map(|name| parse_signature(cx.owner, name))
            .collect::<anyhow::Result<Vec<_>>>()?,
        form_ids: action
            .form_ids
            .iter()
            .map(|value| parse_form_id(cx.owner, value, own_index))
            .collect::<anyhow::Result<Vec<_>>>()?,
    };

    let report = plugin.prune(&request);

    // Asking for something that is not there means the modlist and the archive
    // disagree, and carrying on would leave the row half-done while the install
    // reported success.
    let missing_groups: Vec<&String> = action
        .groups
        .iter()
        .zip(request.groups.iter())
        .filter(|(_, signature)| !report.groups.contains(signature))
        .map(|(name, _)| name)
        .collect();
    let missing_ids: Vec<&String> = action
        .form_ids
        .iter()
        .zip(request.form_ids.iter())
        .filter(|(_, form_id)| !report.form_ids.contains(form_id))
        .map(|(name, _)| name)
        .collect();
    if !missing_groups.is_empty() || !missing_ids.is_empty() {
        anyhow::bail!(
            "{}: delete_records found nothing to remove for {}{}{} in {}",
            cx.owner,
            if missing_groups.is_empty() {
                String::new()
            } else {
                format!("group(s) {missing_groups:?}")
            },
            if missing_groups.is_empty() || missing_ids.is_empty() {
                ""
            } else {
                " and "
            },
            if missing_ids.is_empty() {
                String::new()
            } else {
                format!("record(s) {missing_ids:?}")
            },
            action.plugin,
        );
    }

    std::fs::write(&path, plugin.to_bytes())
        .map_err(|err| anyhow::anyhow!("{}: failed to write {}: {err}", cx.owner, path.display()))?;

    tracing::info!(
        owner = cx.owner,
        plugin = %action.plugin,
        groups = ?action.groups,
        form_ids = ?action.form_ids,
        entries_removed = report.entries_removed,
        "delete_records: removed"
    );
    Ok(())
}

fn parse_signature(owner: &str, name: &str) -> anyhow::Result<[u8; 4]> {
    let bytes = name.as_bytes();
    if bytes.len() != 4 {
        anyhow::bail!(
            "{owner}: delete_records group '{name}' is not a four-character record signature, \
             such as WRLD or CELL"
        );
    }
    Ok([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// `xx` for the plugin's own mod index, as xEdit shows it and the guide writes
/// it. Anything else is eight plain hex digits.
fn parse_form_id(owner: &str, value: &str, own_index: u8) -> anyhow::Result<FormId> {
    let trimmed = value.trim();
    let (index, rest) = if let Some(rest) = trimmed
        .strip_prefix("xx")
        .or_else(|| trimmed.strip_prefix("XX"))
    {
        (own_index, rest)
    } else if trimmed.len() == 8 {
        let index = u8::from_str_radix(&trimmed[..2], 16).map_err(|err| {
            anyhow::anyhow!("{owner}: delete_records form_id '{value}' is not hex: {err}")
        })?;
        (index, &trimmed[2..])
    } else {
        anyhow::bail!(
            "{owner}: delete_records form_id '{value}' should be eight hex digits, or 'xx' and \
             six for a record the plugin defines itself"
        );
    };

    if rest.len() != 6 {
        anyhow::bail!(
            "{owner}: delete_records form_id '{value}' should have six hex digits after the mod \
             index"
        );
    }
    let object = u32::from_str_radix(rest, 16).map_err(|err| {
        anyhow::anyhow!("{owner}: delete_records form_id '{value}' is not hex: {err}")
    })?;

    Ok(FormId((u32::from(index) << 24) | object))
}
