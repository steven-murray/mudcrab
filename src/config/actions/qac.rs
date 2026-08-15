//! `qac`: run xEdit's QuickAutoClean over the mod's plugins.

use super::ActionCx;
use crate::config::schema::QacAction;

pub(super) fn apply(action: &QacAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: qac is only valid as a per-mod action", cx.owner);
    };
    crate::config::tools::xedit::apply_qac_action(cx.owner, action, cx.settings, mod_target)
}
