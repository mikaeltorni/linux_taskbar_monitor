//! Override `_getUsageColor` with 256-step RGB gradients.
//!
//! Patches `extension.js` to:
//!
//! 1. Inject gradient configuration into each indicator's color settings during
//!    init. Each colors array gets a type marker (e.g. `"__cpu"`) appended so
//!    the gradient function can reliably identify the metric being colored.
//! 2. Override `_getUsageColor` with a gradient-based implementation that uses
//!    those markers to select the correct gradient.
//!
//! The gradient ranges come from [`crate::gradient_colors`] so the values
//! interpolated into the injected support block have a single definition. The
//! math itself is duplicated as a self-contained JavaScript block because GNOME
//! Shell (GJS) cannot import Rust or Node modules at runtime.

use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::gradient_colors::{
    DISK_USAGE_MAX_PERCENT, ETHERNET_MAX_MBPS, GPU_MEMORY_MAX_GB, RAM_MAX_GB,
};
use crate::logging;

/// Presence of this identifier means the gradient patch is already applied.
const ALREADY_PATCHED_MARKER: &str = "_gradientGetUsageColor";

/// Anchor the support block is injected before.
const CLASS_MARKER: &str = "export default class";

const ORIGINAL_METHOD: &str = r#"    _getUsageColor(value, colors) {
      return getUsageColor(value, colors, COLOR_LIST_SEPARATOR);
    }"#;

/// Everything in the injected support block after the four range constants.
const SUPPORT_BLOCK_BODY: &str = r#"
function rgbToStyle(r, g, b) {
  const rr = Math.max(0, Math.min(255, Math.round(r)));
  const gg = Math.max(0, Math.min(255, Math.round(g)));
  const bb = Math.max(0, Math.min(255, Math.round(b)));
  return `color: rgb(${rr}, ${gg}, ${bb});`;
}

function getGradientColor(value, minVal, maxVal, startRGB, endRGB) {
  if (!Number.isFinite(value)) return "";

  const ratio = (value - minVal) / (maxVal - minVal);
  const clampedRatio = Math.max(0, Math.min(1, ratio));
  const r = startRGB[0] + (endRGB[0] - startRGB[0]) * clampedRatio;
  const g = startRGB[1] + (endRGB[1] - startRGB[1]) * clampedRatio;
  const b = startRGB[2] + (endRGB[2] - startRGB[2]) * clampedRatio;

  return rgbToStyle(r, g, b);
}

function getGreenYellowRedGradientColor(value, minVal, maxVal, startRGB, endRGB) {
  const midpoint = minVal + (maxVal - minVal) / 2;
  const midpointRGB = [255, 255, 0];

  if (value <= midpoint) {
    return getGradientColor(value, minVal, midpoint, startRGB, midpointRGB);
  }

  return getGradientColor(value, midpoint, maxVal, midpointRGB, endRGB);
}

const GRADIENT_CONFIGS = {
  cpu: {
    minVal: 0,
    maxVal: 100,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
  ram: {
    minVal: 0,
    maxVal: RAM_MAX_GB,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
  diskSpace: {
    minVal: 0,
    maxVal: DISK_USAGE_MAX_PERCENT,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
  eth: {
    minVal: 0,
    maxVal: ETHERNET_MAX_MBPS,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
  wlan: {
    minVal: 0,
    maxVal: ETHERNET_MAX_MBPS,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
  gpu: {
    minVal: 0,
    maxVal: 100,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
  gpuMemory: {
    minVal: 0,
    maxVal: GPU_MEMORY_MAX_GB,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
};"#;

/// Gradient implementation plus type-marker injection, replacing the upstream
/// `_getUsageColor` method body.
const REPLACEMENT: &str = r#"    // ── Gradient-based color override (patched by rm-monitor patch-colors) ──
    // Augment each indicator's colors array with a type marker for reliable detection.
    _augmentColorsWithType(colors, typeMarker) {
      if (!Array.isArray(colors)) return colors;
      const hasMarker = colors.some(c => c.includes(typeMarker));
      if (hasMarker) return colors;
      return [...colors, typeMarker];
    }

    // Override all color initialization to inject type markers.
    _diskSpaceColorsChanged() {
      this._diskSpaceColors = this._settings.get_strv(DISK_SPACE_COLORS);
      this._diskSpaceColors = this._augmentColorsWithType(this._diskSpaceColors, "__diskSpace");
    }

    _cpuColorsChanged() {
      this._cpuColors = this._settings.get_strv(CPU_COLORS);
      this._cpuColors = this._augmentColorsWithType(this._cpuColors, "__cpu");
    }

    _ramColorsChanged() {
      this._ramColors = this._settings.get_strv(RAM_COLORS);
      this._ramColors = this._augmentColorsWithType(this._ramColors, "__ram");
    }

    _netEthColorsChanged() {
      this._netEthColors = this._settings.get_strv(NET_ETH_COLORS);
      this._netEthColors = this._augmentColorsWithType(this._netEthColors, "__eth");
    }

    _netWlanColorsChanged() {
      this._netWlanColors = this._settings.get_strv(NET_WLAN_COLORS);
      this._netWlanColors = this._augmentColorsWithType(this._netWlanColors, "__wlan");
    }

    _gpuColorsChanged() {
      this._gpuColors = this._settings.get_strv(GPU_COLORS);
      this._gpuColors = this._augmentColorsWithType(this._gpuColors, "__gpu");
    }

    _gpuMemoryColorsChanged() {
      this._gpuMemoryColors = this._settings.get_strv(GPU_MEMORY_COLORS);
      this._gpuMemoryColors = this._augmentColorsWithType(this._gpuMemoryColors, "__gpuMem");
    }

    // Override the main color initialization to inject type markers.
    _initColors() {
      this._cpuColors = this._augmentColorsWithType(
        this._settings.get_strv(CPU_COLORS), "__cpu"
      );
      this._ramColors = this._augmentColorsWithType(
        this._settings.get_strv(RAM_COLORS), "__ram"
      );
      this._diskSpaceColors = this._augmentColorsWithType(
        this._settings.get_strv(DISK_SPACE_COLORS), "__diskSpace"
      );
      this._netEthColors = this._augmentColorsWithType(
        this._settings.get_strv(NET_ETH_COLORS), "__eth"
      );
      this._netWlanColors = this._augmentColorsWithType(
        this._settings.get_strv(NET_WLAN_COLORS), "__wlan"
      );
      this._gpuColors = this._augmentColorsWithType(
        this._settings.get_strv(GPU_COLORS), "__gpu"
      );
      this._gpuMemoryColors = this._augmentColorsWithType(
        this._settings.get_strv(GPU_MEMORY_COLORS), "__gpuMem"
      );
    }

    // ── Gradient-based _getUsageColor implementation ──
    _gradientGetUsageColor(value, colors) {
      const numericValue = Array.isArray(value) ? Math.max(...value.filter(Number.isFinite)) : value;
      if (!Number.isFinite(numericValue)) return "";

      // Detect indicator type from the colors parameter's type markers.
      const colorStr = Array.isArray(colors) ? colors.join(" ") : String(colors || "");

      let config;
      if (colors === this._diskSpaceColors) {
        config = GRADIENT_CONFIGS.diskSpace;
      } else if (colors === this._netEthColors || colorStr.includes("__eth")) {
        config = GRADIENT_CONFIGS.eth;
      } else if (colors === this._netWlanColors || colorStr.includes("__wlan")) {
        config = GRADIENT_CONFIGS.wlan;
      } else if (colors === this._gpuMemoryColors || colorStr.includes("__gpuMem")) {
        config = GRADIENT_CONFIGS.gpuMemory;
      } else if (colorStr.includes("__diskSpace")) {
        config = GRADIENT_CONFIGS.diskSpace;
      } else if (colors === this._gpuColors || colorStr.includes("__gpu")) {
        config = GRADIENT_CONFIGS.gpu;
      } else if (colors === this._ramColors || colorStr.includes("__ram")) {
        config = GRADIENT_CONFIGS.ram;
      } else if (colors === this._cpuColors || colorStr.includes("__cpu")) {
        config = GRADIENT_CONFIGS.cpu;
      } else {
        config = GRADIENT_CONFIGS.cpu;
      }

      let effectiveValue = numericValue;
      let minVal = config.minVal;
      let maxVal = config.maxVal;

      if (config.inverted) {
        effectiveValue = maxVal - numericValue;
      }

      return getGreenYellowRedGradientColor(
        effectiveValue,
        minVal,
        maxVal,
        config.startRGB,
        config.endRGB
      );
    }

    _getUsageColor(value, colors) {
      return this._gradientGetUsageColor(value, colors);
    }"#;

const HELPER_INSERTION_POINT: &str = "  return rgbToStyle(r, g, b);\n}";

const HELPER_BLOCK: &str = r#"  return rgbToStyle(r, g, b);
}

function getGreenYellowRedGradientColor(value, minVal, maxVal, startRGB, endRGB) {
  const midpoint = minVal + (maxVal - minVal) / 2;
  const midpointRGB = [255, 255, 0];

  if (value <= midpoint) {
    return getGradientColor(value, minVal, midpoint, startRGB, midpointRGB);
  }

  return getGradientColor(value, midpoint, maxVal, midpointRGB, endRGB);
}"#;

const OLD_RETURN: &str =
    "      return getGradientColor(effectiveValue, minVal, maxVal, config.startRGB, config.endRGB);";

const NEW_RETURN: &str = r#"      return getGreenYellowRedGradientColor(
        effectiveValue,
        minVal,
        maxVal,
        config.startRGB,
        config.endRGB
      );"#;

const MARKER_ONLY_DETECTION: &str = r#"      if (colorStr.includes("__eth")) {
        config = GRADIENT_CONFIGS.eth;
      } else if (colorStr.includes("__wlan")) {
        config = GRADIENT_CONFIGS.wlan;
      } else if (colorStr.includes("__gpuMem")) {
        config = GRADIENT_CONFIGS.gpuMemory;
      } else if (colorStr.includes("__diskSpace")) {
        config = GRADIENT_CONFIGS.diskSpace;
      } else if (colorStr.includes("__gpu")) {
        config = GRADIENT_CONFIGS.gpu;
      } else if (colorStr.includes("__ram")) {
        config = GRADIENT_CONFIGS.ram;
      } else if (colorStr.includes("__cpu")) {
        config = GRADIENT_CONFIGS.cpu;"#;

const IDENTITY_DETECTION: &str = r#"      if (colors === this._diskSpaceColors) {
        config = GRADIENT_CONFIGS.diskSpace;
      } else if (colors === this._netEthColors || colorStr.includes("__eth")) {
        config = GRADIENT_CONFIGS.eth;
      } else if (colors === this._netWlanColors || colorStr.includes("__wlan")) {
        config = GRADIENT_CONFIGS.wlan;
      } else if (colors === this._gpuMemoryColors || colorStr.includes("__gpuMem")) {
        config = GRADIENT_CONFIGS.gpuMemory;
      } else if (colorStr.includes("__diskSpace")) {
        config = GRADIENT_CONFIGS.diskSpace;
      } else if (colors === this._gpuColors || colorStr.includes("__gpu")) {
        config = GRADIENT_CONFIGS.gpu;
      } else if (colors === this._ramColors || colorStr.includes("__ram")) {
        config = GRADIENT_CONFIGS.ram;
      } else if (colors === this._cpuColors || colorStr.includes("__cpu")) {
        config = GRADIENT_CONFIGS.cpu;"#;

/// Build the self-contained gradient support block injected into `extension.js`.
///
/// The four range constants come from [`crate::gradient_colors`] so the Rust
/// tests and the injected JavaScript can never disagree.
pub fn support_block() -> String {
    format!(
        "// ── Gradient color support (patched by rm-monitor patch-colors) ──\n\
         const ETHERNET_MAX_MBPS = {ETHERNET_MAX_MBPS};\n\
         const RAM_MAX_GB = {RAM_MAX_GB};\n\
         const DISK_USAGE_MAX_PERCENT = {DISK_USAGE_MAX_PERCENT};\n\
         const GPU_MEMORY_MAX_GB = {GPU_MEMORY_MAX_GB};\n{SUPPORT_BLOCK_BODY}"
    )
}

/// Upgrade a previously patched `extension.js` from the older two-stop gradient
/// to the green -> yellow -> red midpoint gradient.
///
/// Injects `getGreenYellowRedGradientColor` when missing and rewrites the
/// `_getUsageColor` return statement to call it. Each substitution is applied
/// only when its source form is present, so this is a no-op on current content.
///
/// # Parameters
/// - `content`: Current `extension.js` content.
pub fn migrate_green_yellow_red_gradient(content: &str) -> String {
    let mut migrated = content.to_string();

    if !migrated.contains("function getGreenYellowRedGradientColor")
        && migrated.contains(HELPER_INSERTION_POINT)
    {
        migrated = migrated.replacen(HELPER_INSERTION_POINT, HELPER_BLOCK, 1);
    }

    if migrated.contains(OLD_RETURN) {
        migrated = migrated.replacen(OLD_RETURN, NEW_RETURN, 1);
    }

    if migrated != content {
        logging::info("Migrated gradient colors to green-yellow-red midpoint");
        println!("Migrated gradient colors to green-yellow-red midpoint");
    }

    migrated
}

/// Failures when the upstream Resource Monitor anchors are missing.
#[derive(Debug, Error, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ColorsError {
    /// `_getUsageColor` method body is absent.
    #[error(
        "Could not find target _getUsageColor in extension.js — unsupported extension version"
    )]
    MissingMethod,
    /// `export default class` marker is absent.
    #[error(
        "Could not find extension class marker in extension.js — unsupported extension version"
    )]
    MissingClass,
    /// Already-patched marker present but identity detection could not migrate.
    #[error(
        "Could not migrate gradient color detection to property identity checks — unsupported extension version"
    )]
    MissingIdentityMigration,
}

/// Patch `extension.js` to override `_getUsageColor` with gradient coloring.
///
/// Also injects type markers into each indicator's color settings during init.
/// When the upstream anchors are missing, returns [`ColorsError`] so the CLI
/// exits non-zero like the other patchers.
///
/// # Parameters
/// - `content`: Original `extension.js` content.
///
/// # Returns
/// `(patched_content, changed)` where `changed` is false when the file already
/// matches the target gradient patch (including after no-op migrations).
pub fn patch_extension_js(content: &str) -> Result<(String, bool), ColorsError> {
    if content.contains(ALREADY_PATCHED_MARKER) {
        let mut migrated = migrate_green_yellow_red_gradient(content);
        if !migrated.contains("colors === this._diskSpaceColors") {
            logging::info("Migrating gradient color detection to property identity checks");
            println!("Migrating gradient color detection to property identity checks");
            if !migrated.contains(MARKER_ONLY_DETECTION) {
                return Err(ColorsError::MissingIdentityMigration);
            }
            migrated = migrated.replacen(MARKER_ONLY_DETECTION, IDENTITY_DETECTION, 1);
            if !migrated.contains("colors === this._diskSpaceColors") {
                return Err(ColorsError::MissingIdentityMigration);
            }
        } else {
            logging::info("Colors already patched — skipping");
            println!("Colors already patched — skipping");
        }
        let changed = migrated != content;
        return Ok((migrated, changed));
    }

    if !content.contains(ORIGINAL_METHOD) {
        return Err(ColorsError::MissingMethod);
    }

    if !content.contains(CLASS_MARKER) {
        return Err(ColorsError::MissingClass);
    }

    let with_support_block = content.replacen(
        CLASS_MARKER,
        &format!("{}\n\n{CLASS_MARKER}", support_block()),
        1,
    );
    let patched = with_support_block.replacen(ORIGINAL_METHOD, REPLACEMENT, 1);
    logging::info("Patched extension.js with gradient-based color system");
    println!("Patched extension.js with gradient-based color system");
    Ok((patched, true))
}

/// CLI entry point for the gradient color patcher.
///
/// # Parameters
/// - `extension_path`: Path to the extension's `extension.js`.
///
/// Returns `0` on success (including already-patched) and `1` when the file
/// cannot be read/written or the target anchors are missing.
pub fn run(extension_path: &Path) -> i32 {
    logging::info(format!("patch-colors path={}", extension_path.display()));

    let content = match fs::read_to_string(extension_path) {
        Ok(content) => content,
        Err(err) => {
            logging::error(format!(
                "Could not read {}: {err}",
                extension_path.display()
            ));
            eprintln!("Could not read {}: {err}", extension_path.display());
            return 1;
        }
    };

    let (patched, changed) = match patch_extension_js(&content) {
        Ok(result) => result,
        Err(err) => {
            logging::error(err.to_string());
            eprintln!("{err}");
            return 1;
        }
    };

    if !changed {
        return 0;
    }

    if let Err(err) = fs::write(extension_path, patched) {
        logging::error(format!(
            "Could not write {}: {err}",
            extension_path.display()
        ));
        eprintln!("Could not write {}: {err}", extension_path.display());
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream() -> String {
        format!(
            "import GObject from \"gi://GObject\";\n\n\
             {CLASS_MARKER} ResourceMonitorExtension extends Extension {{\n\
             {ORIGINAL_METHOD}\n}}\n"
        )
    }

    #[test]
    fn support_block_carries_the_shared_range_constants() {
        let block = support_block();
        assert!(block.contains("const ETHERNET_MAX_MBPS = 2000;"));
        assert!(block.contains("const RAM_MAX_GB = 64;"));
        assert!(block.contains("const DISK_USAGE_MAX_PERCENT = 100;"));
        assert!(block.contains("const GPU_MEMORY_MAX_GB = 24;"));
        assert!(block.contains("function getGreenYellowRedGradientColor("));
        assert!(block.ends_with("};"));
    }

    #[test]
    fn patches_upstream_extension_source() {
        let (patched, changed) = patch_extension_js(&upstream()).expect("patched");
        assert!(changed);
        assert!(patched.contains("_gradientGetUsageColor"));
        assert!(patched.contains("return this._gradientGetUsageColor(value, colors);"));
        assert!(patched.contains("colors === this._diskSpaceColors"));
        assert!(patched.contains("const GRADIENT_CONFIGS = {"));
        // The support block must precede the class declaration.
        let block_at = patched.find("Gradient color support").expect("support block");
        let class_at = patched.find(CLASS_MARKER).expect("class marker");
        assert!(block_at < class_at);
    }

    #[test]
    fn re_running_the_patch_is_a_no_op() {
        let (once, _) = patch_extension_js(&upstream()).expect("first");
        let (twice, changed) = patch_extension_js(&once).expect("second");
        assert!(!changed);
        assert_eq!(once, twice);
    }

    #[test]
    fn missing_anchors_are_rejected() {
        assert_eq!(
            patch_extension_js("// unrelated"),
            Err(ColorsError::MissingMethod)
        );
        let no_class = ORIGINAL_METHOD.to_string();
        assert_eq!(
            patch_extension_js(&no_class),
            Err(ColorsError::MissingClass)
        );
    }

    #[test]
    fn migrates_marker_only_detection_to_identity_checks() {
        let legacy = format!(
            "_gradientGetUsageColor(value, colors) {{\n{MARKER_ONLY_DETECTION}\n      }}\n{NEW_RETURN}\n\
             function getGreenYellowRedGradientColor(a) {{}}\n"
        );
        let (migrated, changed) = patch_extension_js(&legacy).expect("migrate");
        assert!(changed);
        assert!(migrated.contains("colors === this._diskSpaceColors"));
        assert!(migrated.contains("colors === this._gpuMemoryColors || colorStr.includes(\"__gpuMem\")"));
    }

    #[test]
    fn mismatched_legacy_detection_fails_hard() {
        let broken = "_gradientGetUsageColor(value, colors) {\n      // unexpected detection body\n      }\n";
        assert_eq!(
            patch_extension_js(broken),
            Err(ColorsError::MissingIdentityMigration)
        );
    }

    #[test]
    fn migrates_the_two_stop_gradient_to_the_midpoint_gradient() {
        let legacy = format!(
            "_gradientGetUsageColor(value, colors) {{\n{IDENTITY_DETECTION}\n      }}\n\
             function getGradientColor(v) {{\n{HELPER_INSERTION_POINT}\n{OLD_RETURN}\n"
        );
        let (migrated, changed) = patch_extension_js(&legacy).expect("migrate");
        assert!(changed);
        assert!(migrated.contains("function getGreenYellowRedGradientColor("));
        assert!(migrated.contains(NEW_RETURN));
        assert!(!migrated.contains(OLD_RETURN));
    }

    #[test]
    fn run_writes_the_patched_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("extension.js");
        fs::write(&path, upstream()).expect("write");
        assert_eq!(run(&path), 0);
        assert!(fs::read_to_string(&path)
            .expect("read")
            .contains("_gradientGetUsageColor"));
        assert_eq!(run(&path), 0);
    }

    #[test]
    fn run_exits_one_for_unsupported_content() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("extension.js");
        fs::write(&path, "// unrelated").expect("write");
        assert_eq!(run(&path), 1);
        assert_eq!(fs::read_to_string(&path).expect("read"), "// unrelated");
    }

    #[test]
    fn run_exits_one_for_a_missing_file() {
        assert_eq!(run(Path::new("/nonexistent/rm-monitor/extension.js")), 1);
    }
}
