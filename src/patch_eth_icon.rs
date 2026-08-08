//! Drop the ethernet display icon from the panel while keeping its Mbps value.
//!
//! Upstream appends the
//! icon for every simple metric group (cpu/ram/swap/disk/eth/wlan) through
//! `_appendSimpleChildren`, and there is no GSetting to hide a single icon, so
//! this patcher:
//!
//! 1. Guards `_appendSimpleChildren` so a null icon adds no actor (St's
//!    `addChild` rejects null).
//! 2. Wires the ethernet group to `_appendSimpleChildren` with a null icon.
//!
//! The edits are idempotent (guarded by marker comments) and fail fast when the
//! expected upstream snippets are absent.

use std::fs;
use std::path::Path;

use crate::logging;

const ICON_GUARD_MARKER: &str = "A null icon (e.g. ethernet) means no icon actor is added";

const OLD_FN: &str = r#"function _appendSimpleChildren(icon, value, unit, addChild, iconsPosition) {
  if (iconsPosition === "left") {
    addChild(icon);
  }

  addChild(value);
  addChild(unit);

  if (iconsPosition !== "left") {
    addChild(icon);
  }
}"#;

const NEW_FN: &str = r#"function _appendSimpleChildren(icon, value, unit, addChild, iconsPosition) {
  // A null icon (e.g. ethernet) means no icon actor is added.
  if (icon) {
    if (iconsPosition === "left") {
      addChild(icon);
    }
  }

  addChild(value);
  addChild(unit);

  if (icon) {
    if (iconsPosition !== "left") {
      addChild(icon);
    }
  }
}"#;

const ETH_MARKER: &str = "Ethernet icon removed: value/unit kept, icon omitted";

const OLD_SNIPPET: &str = r#"  _replaceGroupChildren(indicator._ethGroup, (addChild) =>
    _appendSimpleChildren(
      indicator._ethIcon,
      indicator._ethValue,
      indicator._ethUnit,
      addChild,
      iconsPosition
    )
  );"#;

const NEW_SNIPPET: &str = r#"  _replaceGroupChildren(indicator._ethGroup, (addChild) =>
    // Ethernet icon removed: value/unit kept, icon omitted
    _appendSimpleChildren(
      null,
      indicator._ethValue,
      indicator._ethUnit,
      addChild,
      iconsPosition
    )
  );"#;

/// Failure while applying the ethernet-icon patch.
#[derive(Debug, thiserror::Error)]
pub enum EthIconError {
    /// `_appendSimpleChildren` could not be located (and the null-icon guard
    /// marker is also absent, so this is not the already-patched case).
    #[error(
        "Could not find _appendSimpleChildren in mainGui.js - unsupported Resource Monitor version"
    )]
    MissingAppendSimpleChildren,
    /// The ethernet group wiring could not be located (and the eth-icon marker
    /// is also absent, so this is not the already-patched case).
    #[error(
        "Could not find ethernet group wiring in mainGui.js - unsupported Resource Monitor version"
    )]
    MissingEthWiring,
}

/// Apply both ethernet-icon edits to `mainGui.js` content.
///
/// # Parameters
/// - `content`: Current `mainGui.js` content.
///
/// Returns the patched content and whether anything changed.
pub fn patch_main_gui(content: &str) -> Result<(String, bool), EthIconError> {
    let mut content = content.to_string();
    let mut changed = false;

    if !content.contains(ICON_GUARD_MARKER) {
        if !content.contains(OLD_FN) {
            return Err(EthIconError::MissingAppendSimpleChildren);
        }
        content = content.replacen(OLD_FN, NEW_FN, 1);
        changed = true;
        logging::info("Guarded _appendSimpleChildren against null icon (eth-icon patch)");
        println!("Guarded _appendSimpleChildren against null icon (eth-icon patch)");
    }

    if !content.contains(ETH_MARKER) {
        if !content.contains(OLD_SNIPPET) {
            return Err(EthIconError::MissingEthWiring);
        }
        content = content.replacen(OLD_SNIPPET, NEW_SNIPPET, 1);
        changed = true;
        logging::info("Removed ethernet display icon (value/unit preserved)");
        println!("Removed ethernet display icon (value/unit preserved)");
    }

    Ok((content, changed))
}

/// CLI entry point for the ethernet-icon patcher.
///
/// # Parameters
/// - `main_gui_path`: Path to the extension's `panel/mainGui.js`.
///
/// Returns `0` on success (including the already-patched case) and `1` when a
/// required upstream snippet is missing or the file cannot be read/written.
pub fn run(main_gui_path: &Path) -> i32 {
    logging::info(format!("patch-eth-icon path={}", main_gui_path.display()));

    let content = match fs::read_to_string(main_gui_path) {
        Ok(content) => content,
        Err(err) => {
            logging::error(format!("Could not read {}: {err}", main_gui_path.display()));
            eprintln!("Could not read {}: {err}", main_gui_path.display());
            return 1;
        }
    };

    let (patched, changed) = match patch_main_gui(&content) {
        Ok(result) => result,
        Err(err) => {
            logging::error(err.to_string());
            eprintln!("{err}");
            return 1;
        }
    };

    if !changed {
        logging::info("Ethernet display icon already removed");
        println!("Ethernet display icon already removed");
        return 0;
    }

    if let Err(err) = crate::patch_text::write_atomic(main_gui_path, &patched) {
        logging::error(format!(
            "Could not write {}: {err}",
            main_gui_path.display()
        ));
        eprintln!("Could not write {}: {err}", main_gui_path.display());
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream() -> String {
        format!("{OLD_FN}\n\n{OLD_SNIPPET}\n")
    }

    #[test]
    fn applies_both_edits_to_upstream_source() {
        let (patched, changed) = patch_main_gui(&upstream()).expect("patch");
        assert!(changed);
        assert!(patched.contains(ICON_GUARD_MARKER));
        assert!(patched.contains(ETH_MARKER));
        assert!(patched.contains("      null,\n      indicator._ethValue,"));
    }

    #[test]
    fn re_running_the_patch_changes_nothing() {
        let (patched, _) = patch_main_gui(&upstream()).expect("first patch");
        let (again, changed) = patch_main_gui(&patched).expect("second patch");
        assert!(!changed);
        assert_eq!(patched, again);
    }

    #[test]
    fn missing_append_simple_children_fails_fast() {
        assert!(matches!(
            patch_main_gui("// unrelated"),
            Err(EthIconError::MissingAppendSimpleChildren)
        ));
    }

    #[test]
    fn missing_eth_wiring_fails_fast() {
        assert!(matches!(
            patch_main_gui(OLD_FN),
            Err(EthIconError::MissingEthWiring)
        ));
    }

    #[test]
    fn run_is_idempotent_on_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("mainGui.js");
        fs::write(&path, upstream()).expect("write");
        assert_eq!(run(&path), 0);
        let first = fs::read_to_string(&path).expect("read");
        assert_eq!(run(&path), 0);
        assert_eq!(first, fs::read_to_string(&path).expect("read"));
    }
}
