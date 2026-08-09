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
//! expected upstream snippets are absent. A guard marker being present does
//! not short-circuit the check: the guarded block's body is compared against
//! the current form and upgraded in place when it has drifted (e.g. from an
//! older codegen pass), and the patch still fails fast if the marker is
//! present but the block's bounds cannot be located.

use std::fs;
use std::path::Path;

use crate::logging;
use crate::patch_text::{self, StaleBlockOutcome};

const ICON_GUARD_MARKER: &str = "A null icon (e.g. ethernet) means no icon actor is added";

/// Signature line that starts the guarded function, shared by [`OLD_FN`] and
/// [`NEW_FN`] so a stale (but marker-carrying) body can still be located.
const FN_SIGNATURE: &str =
    "function _appendSimpleChildren(icon, value, unit, addChild, iconsPosition) {";

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

/// Call-site line that starts the ethernet wiring block, shared by
/// [`OLD_SNIPPET`] and [`NEW_SNIPPET`] so a stale (but marker-carrying) body
/// can still be located.
const ETH_SIGNATURE: &str = "  _replaceGroupChildren(indicator._ethGroup, (addChild) =>";

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

    if content.contains(ICON_GUARD_MARKER) {
        // Marker present does not guarantee the guarded body is current — a
        // stale patched form (e.g. from an older codegen pass) can still
        // carry the marker. Locate the guarded function by its stable
        // signature and balanced braces, and upgrade it in place when it
        // differs from the current NEW_FN.
        match patch_text::replace_stale_balanced_block(
            &content,
            FN_SIGNATURE,
            '{',
            '}',
            None,
            NEW_FN,
        ) {
            StaleBlockOutcome::Replaced(updated) => {
                content = updated;
                changed = true;
                logging::info("Upgraded stale _appendSimpleChildren null-icon guard body");
                println!("Upgraded stale _appendSimpleChildren null-icon guard body");
            }
            StaleBlockOutcome::Unchanged => {}
            StaleBlockOutcome::AnchorsMissing => {
                return Err(EthIconError::MissingAppendSimpleChildren);
            }
        }
    } else {
        if !content.contains(OLD_FN) {
            return Err(EthIconError::MissingAppendSimpleChildren);
        }
        content = content.replacen(OLD_FN, NEW_FN, 1);
        changed = true;
        logging::info("Guarded _appendSimpleChildren against null icon (eth-icon patch)");
        println!("Guarded _appendSimpleChildren against null icon (eth-icon patch)");
    }

    if content.contains(ETH_MARKER) {
        // Same rationale as above: the marker can survive a stale wiring
        // body, so locate the balanced `_replaceGroupChildren(...)` call by
        // its stable signature and upgrade it when it differs from
        // NEW_SNIPPET.
        match patch_text::replace_stale_balanced_block(
            &content,
            ETH_SIGNATURE,
            '(',
            ')',
            Some(';'),
            NEW_SNIPPET,
        ) {
            StaleBlockOutcome::Replaced(updated) => {
                content = updated;
                changed = true;
                logging::info("Upgraded stale ethernet display icon wiring");
                println!("Upgraded stale ethernet display icon wiring");
            }
            StaleBlockOutcome::Unchanged => {}
            StaleBlockOutcome::AnchorsMissing => {
                return Err(EthIconError::MissingEthWiring);
            }
        }
    } else {
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
    fn stale_guard_body_is_upgraded_when_marker_present() {
        let (once, _) = patch_main_gui(&upstream()).expect("first patch");
        // Simulate a stale patched form: marker present, but the guard body
        // dropped the inner `if` around the trailing addChild(icon) call.
        let stale = once.replacen(
            "  if (icon) {\n    if (iconsPosition !== \"left\") {\n      addChild(icon);\n    }\n  }\n}",
            "  if (iconsPosition !== \"left\") {\n    addChild(icon);\n  }\n}",
            1,
        );
        assert!(stale.contains(ICON_GUARD_MARKER));
        assert_ne!(stale, once);

        let (upgraded, changed) = patch_main_gui(&stale).expect("upgrade");
        assert!(changed);
        assert_eq!(upgraded, once);

        let (again, changed_again) = patch_main_gui(&upgraded).expect("idempotent");
        assert!(!changed_again);
        assert_eq!(upgraded, again);
    }

    #[test]
    fn stale_eth_wiring_body_is_upgraded_when_marker_present() {
        let (once, _) = patch_main_gui(&upstream()).expect("first patch");
        // Simulate a stale patched form: marker present, but the wiring
        // still routed through the icon parameter instead of null.
        let stale = once.replacen(
            "    _appendSimpleChildren(\n      null,\n      indicator._ethValue,",
            "    _appendSimpleChildren(\n      indicator._ethIcon,\n      indicator._ethValue,",
            1,
        );
        assert!(stale.contains(ETH_MARKER));
        assert_ne!(stale, once);

        let (upgraded, changed) = patch_main_gui(&stale).expect("upgrade");
        assert!(changed);
        assert_eq!(upgraded, once);

        let (again, changed_again) = patch_main_gui(&upgraded).expect("idempotent");
        assert!(!changed_again);
        assert_eq!(upgraded, again);
    }

    #[test]
    fn marker_present_without_locatable_bounds_fails_hard() {
        // The marker is present but the block's own signature line is gone
        // (e.g. an unsupported upstream rewrite), so bounds cannot be found.
        let broken_fn = format!("// {ICON_GUARD_MARKER}\n{OLD_SNIPPET}\n");
        assert!(matches!(
            patch_main_gui(&broken_fn),
            Err(EthIconError::MissingAppendSimpleChildren)
        ));

        let broken_eth = format!("{NEW_FN}\n// {ETH_MARKER}\n");
        assert!(matches!(
            patch_main_gui(&broken_eth),
            Err(EthIconError::MissingEthWiring)
        ));
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
