//! Plugin identification and staging helpers.

use crate::config::mo2::MO2_HIDDEN_SUFFIX;
use std::path::Path;

/// Is this a plugin the game should load?
///
/// Explicitly excludes MO2's hidden suffix. An extension check alone already
/// happens to reject `Foo.esp.mohidden` (its extension is `mohidden`), but
/// relying on that accident is fragile -- once merges start hiding their source
/// plugins, "hidden plugins are not loaded" becomes load-bearing behaviour and
/// should be stated, not inferred.
pub(crate) fn is_plugin_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if name.to_ascii_lowercase().ends_with(MO2_HIDDEN_SUFFIX) {
        return false;
    }

    let lower = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    lower == "esp" || lower == "esm"
}

