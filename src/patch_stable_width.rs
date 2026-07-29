//! Reserve (or release) fixed value-label widths so the panel does not shift as
//! metric values change digit count.
//!
//! Port of `scripts/patch_resource_monitor_stable_width.js`. The extension's
//! upstream `*width` GSettings already reserve the primary value labels, but two
//! labels have no such GSetting and are left adaptive: the disk-space secondary
//! "activity %" value, and the GPU VRAM value (upstream shares the single
//! `gpuwidth` with the GPU usage percentage). This patcher reserves both through
//! the same mechanism the extension uses for the primary values — setting
//! `element.width`, which St honors — rather than a CSS `min-width` rule, which
//! St does not reliably apply to `St.Label` actors.
//!
//! Modes:
//! - `stable` (default): apply the fixed-width reservations.
//! - `compact`: remove them so the value labels are adaptive again, taking less
//!   horizontal space at the cost of slight panel shifting.

use std::fs;
use std::path::Path;

use crate::logging;

/// Accepted `--mode` values, in the order the usage message lists them.
pub const MODES: [&str; 2] = ["stable", "compact"];

const DISK_MARKER: &str =
    "Space separator between disk-space activity percent and its unit (stable width)";

const GPU_MARKER: &str = "VRAM value (0-99 GB, 2 digits) gets its own tighter reserved";

const DISK_UPSTREAM_ADD: &str = r#"      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);"#;

const DISK_PATCHED_ADD: &str = r#"      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      // Space separator between disk-space activity percent and its unit (stable width)
      this._elementsSecondaryValue[filesystem].width = this._diskActivityWidth;
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);"#;

const DISK_UPSTREAM_INIT: &str = r#"  class DiskContainerSpace extends DiskContainer {
    _init() {
      super._init();

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }"#;

const DISK_PATCHED_INIT: &str = r#"  class DiskContainerSpace extends DiskContainer {
    _init() {
      super._init();

      // Stable reserved width (px, pre-scale) for the disk-space secondary
      // activity percentage so it does not shift the panel as digits change.
      // Covers the 3-digit worst case "100" at the panel font with slack;
      // secondary values render at 0.92em so this comfortably fits it.
      this._diskActivityWidth = 24;

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }"#;

const GPU_UPSTREAM_INIT: &str = r#"  class GpuContainer extends St.BoxLayout {
    _init() {
      super._init();

      this._elementsUuid = [];
      this._elementsName = [];
      this._elementsValue = [];
      this._elementsUnit = [];
      this._elementsMemoryValue = [];
      this._elementsMemoryUnit = [];
      this._elementsThermalValue = [];
      this._elementsThermalUnit = [];
      this._separatorPairs = [];
    }"#;

const GPU_PATCHED_INIT: &str = r#"  class GpuContainer extends St.BoxLayout {
    _init() {
      super._init();

      this._elementsUuid = [];
      this._elementsName = [];
      this._elementsValue = [];
      this._elementsUnit = [];
      this._elementsMemoryValue = [];
      this._elementsMemoryUnit = [];
      this._elementsThermalValue = [];
      this._elementsThermalUnit = [];
      // Stable reserved width (px, pre-scale) for the GPU VRAM value so it
      // stays snug beside the GPU usage percentage. VRAM is numeric GB, at
      // most 2 digits ("99"), measured ~16px at the panel font; the value
      // label is separate from its unit, so this covers the widest reading.
      this._gpuMemoryWidth = 16;
      this._separatorPairs = [];
    }"#;

const GPU_UPSTREAM_SET: &str = r#"      } else {
        this._elementsUuid.forEach((element) => {
          if (this._elementsValue[element] !== undefined) {
            this._elementsValue[element].width = width;
          }

          if (this._elementsMemoryValue[element] !== undefined) {
            this._elementsMemoryValue[element].width = width;
          }
        });
      }"#;

const GPU_PATCHED_SET: &str = r#"      } else {
        this._elementsUuid.forEach((element) => {
          if (this._elementsValue[element] !== undefined) {
            this._elementsValue[element].width = width;
          }

          if (this._elementsMemoryValue[element] !== undefined) {
            // VRAM value (0-99 GB, 2 digits) gets its own tighter reserved
            // width so it stays snug next to the GPU usage percentage.
            this._elementsMemoryValue[element].width = this._gpuMemoryWidth;
          }
        });
      }"#;

/// Failure while applying or reverting the stable-width reservations.
#[derive(Debug, thiserror::Error)]
pub enum StableWidthError {
    /// The compact-mode revert could not find the disk-space edit to remove.
    #[error("Could not find disk-space stable-width edit to remove in containers.js")]
    MissingDiskCompactAdd,
    /// The compact-mode revert could not find the disk width constant.
    #[error("Could not find DiskContainerSpace._init stable-width constant to remove")]
    MissingDiskCompactInit,
    /// The stable-mode patch could not find the disk-space `add_element` body.
    #[error(
        "Could not find disk-space target code in containers.js - patch may be already applied or unsupported version"
    )]
    MissingDiskAdd,
    /// The stable-mode patch could not find `DiskContainerSpace._init`.
    #[error("Could not find DiskContainerSpace._init in containers.js")]
    MissingDiskInit,
    /// The compact-mode revert could not find the VRAM width assignment.
    #[error("Could not find GpuContainer.set_element_width VRAM edit to remove")]
    MissingGpuCompactSet,
    /// The compact-mode revert could not find the VRAM width constant.
    #[error("Could not find GpuContainer._init VRAM width constant to remove")]
    MissingGpuCompactInit,
    /// The stable-mode patch could not find `GpuContainer._init`.
    #[error("Could not find GpuContainer._init in containers.js")]
    MissingGpuInit,
    /// The stable-mode patch could not find `set_element_width`'s else branch.
    #[error("Could not find GpuContainer.set_element_width else-branch in containers.js")]
    MissingGpuSet,
}

fn replace_or(
    content: &str,
    from: &str,
    to: &str,
    error: StableWidthError,
) -> Result<String, StableWidthError> {
    if !content.contains(from) {
        return Err(error);
    }
    Ok(content.replacen(from, to, 1))
}

/// Apply (or revert) both stable-width reservations in `containers.js` content.
///
/// # Parameters
/// - `content`: Current `containers.js` content.
/// - `compact`: When true, release the reservations instead of applying them.
///
/// Returns the patched content and whether anything changed.
pub fn patch_containers(
    content: &str,
    compact: bool,
) -> Result<(String, bool), StableWidthError> {
    let mut content = content.to_string();
    let mut changed = false;

    // ── 1. Disk-space secondary activity % width ────────────────────────────
    if compact {
        if content.contains(DISK_MARKER) {
            content = replace_or(
                &content,
                DISK_PATCHED_ADD,
                DISK_UPSTREAM_ADD,
                StableWidthError::MissingDiskCompactAdd,
            )?;
            content = replace_or(
                &content,
                DISK_PATCHED_INIT,
                DISK_UPSTREAM_INIT,
                StableWidthError::MissingDiskCompactInit,
            )?;
            changed = true;
            logging::info("Released disk-space activity percent width (compact mode)");
            println!("Released disk-space activity percent width (compact mode)");
        } else {
            println!("Disk-space activity percent width already compact");
        }
    } else if !content.contains(DISK_MARKER) {
        content = replace_or(
            &content,
            DISK_UPSTREAM_ADD,
            DISK_PATCHED_ADD,
            StableWidthError::MissingDiskAdd,
        )?;
        content = replace_or(
            &content,
            DISK_UPSTREAM_INIT,
            DISK_PATCHED_INIT,
            StableWidthError::MissingDiskInit,
        )?;
        changed = true;
        logging::info("Reserved disk-space activity percent width (stable-width patch)");
        println!("Reserved disk-space activity percent width (stable-width patch)");
    }

    // ── 2. GPU VRAM width split from GPU usage ──────────────────────────────
    if compact {
        if content.contains(GPU_MARKER) {
            content = replace_or(
                &content,
                GPU_PATCHED_SET,
                GPU_UPSTREAM_SET,
                StableWidthError::MissingGpuCompactSet,
            )?;
            content = replace_or(
                &content,
                GPU_PATCHED_INIT,
                GPU_UPSTREAM_INIT,
                StableWidthError::MissingGpuCompactInit,
            )?;
            changed = true;
            logging::info("Released GPU VRAM width back to shared GPU usage width (compact mode)");
            println!("Released GPU VRAM width back to shared GPU usage width (compact mode)");
        } else {
            println!("GPU VRAM width already compact");
        }
    } else if !content.contains(GPU_MARKER) {
        // The class header makes the init snippet unique: the same field list
        // appears in cleanup_elements without the class declaration prefix.
        content = replace_or(
            &content,
            GPU_UPSTREAM_INIT,
            GPU_PATCHED_INIT,
            StableWidthError::MissingGpuInit,
        )?;
        content = replace_or(
            &content,
            GPU_UPSTREAM_SET,
            GPU_PATCHED_SET,
            StableWidthError::MissingGpuSet,
        )?;
        changed = true;
        logging::info("Split GPU usage / VRAM reserved widths (stable-width patch)");
        println!("Split GPU usage / VRAM reserved widths (stable-width patch)");
    }

    Ok((content, changed))
}

/// CLI entry point for the stable-width patcher.
///
/// # Parameters
/// - `mode`: `"stable"` (reserve widths) or `"compact"` (release them).
/// - `containers_path`: Path to the extension's `panel/containers.js`.
///
/// Returns `0` on success and `1` on an invalid mode, an unreadable/unwritable
/// file, or a missing upstream snippet.
pub fn run(mode: &str, containers_path: &Path) -> i32 {
    logging::info(format!(
        "patch-stable-width mode={mode} path={}",
        containers_path.display()
    ));

    if !MODES.contains(&mode) {
        let message = format!("Invalid --mode \"{mode}\". Use one of: {}", MODES.join(", "));
        logging::error(&message);
        eprintln!("{message}");
        return 1;
    }
    let compact = mode == "compact";

    let content = match fs::read_to_string(containers_path) {
        Ok(content) => content,
        Err(err) => {
            logging::error(format!(
                "Could not read {}: {err}",
                containers_path.display()
            ));
            eprintln!("Could not read {}: {err}", containers_path.display());
            return 1;
        }
    };

    let (patched, changed) = match patch_containers(&content, compact) {
        Ok(result) => result,
        Err(err) => {
            logging::error(err.to_string());
            eprintln!("{err}");
            return 1;
        }
    };

    if !changed {
        let message = if compact {
            "Stable widths already compact"
        } else {
            "Stable widths already reserved"
        };
        logging::info(message);
        println!("{message}");
    }

    if let Err(err) = fs::write(containers_path, patched) {
        logging::error(format!(
            "Could not write {}: {err}",
            containers_path.display()
        ));
        eprintln!("Could not write {}: {err}", containers_path.display());
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream() -> String {
        format!("{DISK_UPSTREAM_INIT}\n\n{DISK_UPSTREAM_ADD}\n\n{GPU_UPSTREAM_INIT}\n\n{GPU_UPSTREAM_SET}\n")
    }

    #[test]
    fn stable_mode_reserves_both_widths() {
        let (patched, changed) = patch_containers(&upstream(), false).expect("patch");
        assert!(changed);
        assert!(patched.contains(DISK_MARKER));
        assert!(patched.contains("this._diskActivityWidth = 24;"));
        assert!(patched.contains(GPU_MARKER));
        assert!(patched.contains("this._gpuMemoryWidth = 16;"));
    }

    #[test]
    fn stable_mode_is_idempotent() {
        let (once, _) = patch_containers(&upstream(), false).expect("first");
        let (twice, changed) = patch_containers(&once, false).expect("second");
        assert!(!changed);
        assert_eq!(once, twice);
    }

    #[test]
    fn compact_mode_round_trips_back_to_upstream() {
        let source = upstream();
        let (stable, _) = patch_containers(&source, false).expect("stable");
        let (compact, changed) = patch_containers(&stable, true).expect("compact");
        assert!(changed);
        assert_eq!(compact, source);
    }

    #[test]
    fn compact_mode_on_upstream_changes_nothing() {
        let source = upstream();
        let (compact, changed) = patch_containers(&source, true).expect("compact");
        assert!(!changed);
        assert_eq!(compact, source);
    }

    #[test]
    fn missing_disk_target_fails_fast() {
        let source = format!("{GPU_UPSTREAM_INIT}\n\n{GPU_UPSTREAM_SET}\n");
        assert!(matches!(
            patch_containers(&source, false),
            Err(StableWidthError::MissingDiskAdd)
        ));
    }

    #[test]
    fn missing_gpu_target_fails_fast() {
        let source = format!("{DISK_UPSTREAM_INIT}\n\n{DISK_UPSTREAM_ADD}\n");
        assert!(matches!(
            patch_containers(&source, false),
            Err(StableWidthError::MissingGpuInit)
        ));
    }

    #[test]
    fn invalid_mode_exits_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("containers.js");
        fs::write(&path, upstream()).expect("write");
        assert_eq!(run("wide", &path), 1);
    }

    #[test]
    fn run_applies_and_reverts_on_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("containers.js");
        let source = upstream();
        fs::write(&path, &source).expect("write");
        assert_eq!(run("stable", &path), 0);
        assert!(fs::read_to_string(&path)
            .expect("read")
            .contains(GPU_MARKER));
        assert_eq!(run("compact", &path), 0);
        assert_eq!(fs::read_to_string(&path).expect("read"), source);
    }
}
