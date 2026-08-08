//! Patch the Resource Monitor disk space display.
//!
//! Three files are touched:
//!
//! - `panel/containers.js`: `DiskContainerSpace` gains a secondary label for
//!   live disk activity, plus styled unit labels.
//! - `services/refreshers.js`: the disk-space row renders free GB coloured by
//!   used percentage, with the live IO busy percentage as the secondary value;
//!   the `getDiskSpaceActivityPercent` and `getDiskUsagePercentStyle` helpers
//!   are injected here.
//! - `extension.js`: disk-space rows are keyed by mount point.
//!
//! Every edit is version-tolerant: known older patched forms are migrated in
//! place, so re-running the patcher across revisions converges instead of
//! failing. The colour math mirrors [`crate::gradient_colors`], which owns the
//! tested Rust definition of the same gradient.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::logging;
use crate::patch_text::{
    replace_known_snippet, replace_known_snippet_optional, replace_known_snippet_or_patterns,
    replace_known_snippet_with_migration, Migration, PatchTargetMissing,
};

// ── containers.js snippets ───────────────────────────────────────────────────

const ORIGINAL_DISK_CONTAINER: &str = r#"export const DiskContainerSpace = GObject.registerClass(
  class DiskContainerSpace extends DiskContainer {
    add_element(filesystem, label) {
      this._elementsPath.push(filesystem);

      this._elementsName[filesystem] = _createNameLabel(label);

      this._elementsValue[filesystem] = _createValueLabel("--");

      this._elementsUnit[filesystem] = _createUnitLabel("GB");

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
    }

    update_element_value(filesystem, value, unit, style = "") {
      if (this._elementsValue[filesystem]) {
        this._elementsValue[filesystem].text = value;
        this._elementsValue[filesystem].style = style;
        this._elementsUnit[filesystem].text = unit;
      }
    }
  }
);"#;

const BROKEN_PATCHED_DISK_CONTAINER: &str = r#"export const DiskContainerSpace = GObject.registerClass(
  class DiskContainerSpace extends DiskContainer {
    add_element(filesystem, label) {
      this._elementsPath.push(filesystem);

      this._elementsName[filesystem] = _createNameLabel(label);

      this._elementsValue[filesystem] = _createValueLabel("--");

      this._elementsUnit[filesystem] = _createUnitLabel("KB");

      // Secondary value label for free space in GB
      this._elementsSecondaryValue[filesystem] = _createValueLabel("", [
        "resource-monitor-secondary-value",
      ]);

      this._elementsSecondaryUnit[filesystem] = _createUnitLabel("GB", [
        "resource-monitor-secondary-unit",
      ]);

      // Space separator between primary and secondary values
      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);
    }

    update_element_value(filesystem, value, unit, style = "") {
      if (this._elementsValue[filesystem]) {
        this._elementsValue[filesystem].text = value;
        this._elementsValue[filesystem].style = style;
        this._elementsUnit[filesystem].text = unit;
      }
    }

    update_element_secondary_value(filesystem, value, unit, style = "") {
      if (this._elementsSecondaryValue[filesystem]) {
        this._elementsSecondaryValue[filesystem].text = value;
        this._elementsSecondaryValue[filesystem].style = style;
        this._elementsSecondaryUnit[filesystem].text = unit;
        this._elementsSecondaryUnit[filesystem].style = style;
      }
    }
  }
);"#;

const FIXED_DISK_CONTAINER: &str = r#"export const DiskContainerSpace = GObject.registerClass(
  class DiskContainerSpace extends DiskContainer {
    _init() {
      super._init();

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }

    add_element(filesystem, label) {
      this._elementsPath.push(filesystem);

      this._elementsName[filesystem] = _createNameLabel(label);

      this._elementsValue[filesystem] = _createValueLabel("--");

      this._elementsUnit[filesystem] = _createUnitLabel("KB");

      this._elementsSecondaryValue[filesystem] = _createValueLabel("", [
        "resource-monitor-secondary-value",
      ]);

      this._elementsSecondaryUnit[filesystem] = _createUnitLabel("%", [
        "resource-monitor-secondary-unit",
      ]);

      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);
    }

    cleanup_elements() {
      super.cleanup_elements();

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }

    update_element_value(filesystem, value, unit, style = "") {
      if (this._elementsValue[filesystem]) {
        this._elementsValue[filesystem].text = value;
        this._elementsValue[filesystem].style = style;
        this._elementsUnit[filesystem].text = unit;
        this._elementsUnit[filesystem].style = style;
      }
    }

    update_element_secondary_value(filesystem, value, unit, style = "") {
      if (this._elementsSecondaryValue[filesystem]) {
        this._elementsSecondaryValue[filesystem].text = value;
        this._elementsSecondaryValue[filesystem].style = style;
        this._elementsSecondaryUnit[filesystem].text = unit;
        this._elementsSecondaryUnit[filesystem].style = style;
      }
    }
  }
);"#;

/// Marker proving `containers.js` already carries the current disk patch
/// (secondary unit is `%`, not the older broken `"GB"` form).
const DISK_CONTAINER_MARKER: &str =
    "this._elementsSecondaryUnit[filesystem] = _createUnitLabel(\"%\", [";

/// Required by both container migrations: the patched `_init` field list.
const SECONDARY_INIT_MARKER: &str = "this._elementsSecondaryValue = [];";

const STYLE_DISK_UNIT_FROM: &str = r#"        this._elementsValue[filesystem].text = value;
        this._elementsValue[filesystem].style = style;
        this._elementsUnit[filesystem].text = unit;"#;

const STYLE_DISK_UNIT_TO: &str = r#"        this._elementsValue[filesystem].text = value;
        this._elementsValue[filesystem].style = style;
        this._elementsUnit[filesystem].text = unit;
        this._elementsUnit[filesystem].style = style;"#;

const STYLE_DISK_SECONDARY_FROM: &str = r#"    update_element_secondary_value(filesystem, value, unit) {
      if (this._elementsSecondaryValue[filesystem]) {
        this._elementsSecondaryValue[filesystem].text = value;
        this._elementsSecondaryUnit[filesystem].text = unit;
      }
    }"#;

const STYLE_DISK_SECONDARY_TO: &str = r#"    update_element_secondary_value(filesystem, value, unit, style = "") {
      if (this._elementsSecondaryValue[filesystem]) {
        this._elementsSecondaryValue[filesystem].text = value;
        this._elementsSecondaryValue[filesystem].style = style;
        this._elementsSecondaryUnit[filesystem].text = unit;
        this._elementsSecondaryUnit[filesystem].style = style;
      }
    }"#;

const UNIT_TEXT_LINE: &str = "        this._elementsUnit[filesystem].text = unit;\n";
const UNIT_STYLE_LINE: &str = "        this._elementsUnit[filesystem].style = style;\n";

// ── refreshers.js snippets ───────────────────────────────────────────────────

const ORIGINAL_REFRESH_UPDATE: &str = r#"          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            `${indicator._getValueFixed(display.value, "diskSpace")}`,
            display.unit,
            indicator._getUsageColor(display.value, indicator._diskSpaceColors)
          );"#;

const BROKEN_PATCHED_REFRESH_UPDATE: &str = r#"          // Primary: used percentage (always shown)
          const primaryValue = entry.usedPercent;
          const primaryUnit = "%";
          const primaryStyle = indicator._getUsageColor(primaryValue, indicator._diskSpaceColors);

          // Secondary: free space in GB (numeric display)
          const factor = getDataScaleFactor(indicator._dataScaleBase || "decimal");
          const freeInGb = entry.availableBytes / (factor * 1024);
          const secondaryValue = Math.round(freeInGb * 10) / 10;
          const secondaryUnit = "GB";

          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            `${indicator._getValueFixed(primaryValue, "diskSpace")}`,
            primaryUnit,
            primaryStyle
          );
          indicator._diskSpaceBox.update_element_secondary_value(
            entry.filesystem,
            secondaryValue.toFixed(1),
            secondaryUnit
          );"#;

const SPACE_USED_PATCHED_REFRESH_UPDATE: &str = r#"          const primaryDisplay = buildDiskSpaceDisplay(entry, {
            monitor: "used",
            unitType: "perc",
            unitMeasure: indicator._diskSpaceUnitMeasure,
            scaleBase: indicator._dataScaleBase,
          });
          const freeDisplay = buildDiskSpaceDisplay(entry, {
            monitor: "free",
            unitType: "numeric",
            unitMeasure: "g",
            scaleBase: indicator._dataScaleBase,
          });
          const primaryStyle = indicator._getUsageColor(
            entry.usedPercent,
            indicator._diskSpaceColors
          );

          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            `${indicator._getValueFixed(primaryDisplay.value, "diskSpace")}`,
            primaryDisplay.unit,
            primaryStyle
          );
          indicator._diskSpaceBox.update_element_secondary_value(
            entry.filesystem,
            `${indicator._getValueFixed(freeDisplay.value, "diskSpace")}`,
            freeDisplay.unit
          );"#;

/// Green -> yellow -> red disk usage colour helper injected into refreshers.js.
/// Mirrors [`crate::gradient_colors::get_disk_usage_percent_style`].
const DISK_USAGE_STYLE_HELPER: &str = r#"function getDiskUsagePercentStyle(value) {
  if (!Number.isFinite(value)) {
    return "";
  }

  const ratio = Math.max(0, Math.min(1, value / 100));
  const red = ratio <= 0.5 ? Math.round(510 * ratio) : 255;
  const green = ratio <= 0.5 ? 255 : Math.round(510 * (1 - ratio));

  return `color: rgb(${red}, ${green}, 0);`;
}"#;

const DISK_ACTIVITY_HELPER_BODY: &str = r#"function getDiskSpaceActivityPercent(indicator, filesystem) {
  if (!indicator._diskSpaceActivitySamples) {
    indicator._diskSpaceActivitySamples = new Map();
  }

  try {
    const [ok, diskStatsContents] = GLib.file_get_contents("/proc/diskstats");
    if (!ok) {
      return 0;
    }

    const diskName = filesystem.replace(/^\/dev\//, "");
    const names = [diskName];
    // LVM/mapper paths show as /dev/mapper/foo while diskstats uses dm-N.
    try {
      const linkTarget = GLib.file_read_link(filesystem);
      if (linkTarget) {
        const base = String(linkTarget).replace(/^.*\//, "");
        if (base && !names.includes(base)) {
          names.push(base);
        }
      }
    } catch (_linkError) {
      // Not a symlink — keep the basename match only.
    }

    const diskStatsLine = new TextDecoder()
      .decode(diskStatsContents)
      .split("\n")
      .map((line) => line.trim().split(/\s+/))
      .find((fields) => fields.length >= 13 && names.includes(fields[2]));

    if (!diskStatsLine) {
      return 0;
    }

    const ioTimeMs = parseInt(diskStatsLine[12], 10);
    if (!Number.isFinite(ioTimeMs)) {
      return 0;
    }

    const sampleTimeMs = GLib.get_monotonic_time() / 1000;
    const previousSample = indicator._diskSpaceActivitySamples.get(filesystem);

    indicator._diskSpaceActivitySamples.set(filesystem, {
      ioTimeMs,
      sampleTimeMs,
    });

    if (!previousSample || !Number.isFinite(previousSample.ioTimeMs)) {
      return 0;
    }

    const elapsedMs = Math.max(0, sampleTimeMs - previousSample.sampleTimeMs);
    const activeMs = Math.max(0, ioTimeMs - previousSample.ioTimeMs);

    return elapsedMs > 0 ? Math.min(100, (activeMs * 100) / elapsedMs) : 0;
  } catch (error) {
    return 0;
  }
}"#;

const ACTIVITY_PERCENT_FREE_GB_REFRESH_UPDATE: &str = r#"          if (!indicator._diskSpaceActivitySamples) {
            indicator._diskSpaceActivitySamples = new Map();
          }

          let activityPercent = 0;
          try {
            const [ok, diskStatsContents] = GLib.file_get_contents("/proc/diskstats");
            if (ok) {
              const diskName = entry.filesystem.replace(/^\/dev\//, "");
              const diskStatsLine = new TextDecoder()
                .decode(diskStatsContents)
                .split("\n")
                .map((line) => line.trim().split(/\s+/))
                .find((fields) => fields.length >= 13 && fields[2] === diskName);

              if (diskStatsLine) {
                const ioTimeMs = parseInt(diskStatsLine[12], 10);
                const sampleTimeMs = GLib.get_monotonic_time() / 1000;
                const previousSample = indicator._diskSpaceActivitySamples.get(
                  entry.filesystem
                );

                indicator._diskSpaceActivitySamples.set(entry.filesystem, {
                  ioTimeMs,
                  sampleTimeMs,
                });

                if (previousSample && Number.isFinite(ioTimeMs)) {
                  const elapsedMs = Math.max(
                    0,
                    sampleTimeMs - previousSample.sampleTimeMs
                  );
                  const activeMs = Math.max(
                    0,
                    ioTimeMs - previousSample.ioTimeMs
                  );
                  activityPercent = elapsedMs > 0
                    ? Math.min(100, (activeMs * 100) / elapsedMs)
                    : 0;
                }
              }
            }
          } catch (error) {
            activityPercent = 0;
          }

          const freeDisplay = buildDiskSpaceDisplay(entry, {
            monitor: "free",
            unitType: "numeric",
            unitMeasure: "g",
            scaleBase: indicator._dataScaleBase,
          });
          const primaryStyle = indicator._getUsageColor(
            activityPercent,
            indicator._diskSpaceColors
          );

          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,
            "%",
            primaryStyle
          );
          indicator._diskSpaceBox.update_element_secondary_value(
            entry.filesystem,
            `${indicator._getValueFixed(freeDisplay.value, "diskSpace")}`,
            freeDisplay.unit
          );"#;

const FREE_GB_PRIMARY_ACTIVITY_REFRESH_UPDATE: &str = r#"          const diskSpaceDisplay = buildDiskSpaceDisplay(entry, {
            monitor: "free",
            unitType: "numeric",
            unitMeasure: "g",
            scaleBase: indicator._dataScaleBase,
          });
          const activityPercent = getDiskSpaceActivityPercent(indicator, entry.devicePath);
          const primaryStyle = indicator._getUsageColor(entry.usedPercent, indicator._diskSpaceColors);

          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            `${indicator._getValueFixed(diskSpaceDisplay.value, "diskSpace")}`,
            diskSpaceDisplay.unit,
            primaryStyle
          );
          indicator._diskSpaceBox.update_element_secondary_value(
            entry.filesystem,
            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,
            "%"
          );"#;

const FIXED_REFRESH_UPDATE: &str = r#"          const diskSpaceUsageDisplay = buildDiskSpaceDisplay(entry, {
            monitor: "free",
            unitType: "numeric",
            unitMeasure: "g",
            scaleBase: indicator._dataScaleBase,
          });
          const activityPercent = getDiskSpaceActivityPercent(indicator, entry.devicePath);
          const primaryStyle = getDiskUsagePercentStyle(entry.usedPercent);
          const activityStyle = getDiskUsagePercentStyle(activityPercent);

          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            `${indicator._getValueFixed(diskSpaceUsageDisplay.value, "diskSpace")}`,
            diskSpaceUsageDisplay.unit,
            primaryStyle
          );
          indicator._diskSpaceBox.update_element_secondary_value(
            entry.filesystem,
            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,
            "%",
            activityStyle
          );"#;

const USED_GB_ACTIVITY_REFRESH_UPDATE: &str = r#"          const diskSpaceDisplay = buildDiskSpaceDisplay(entry, {
            monitor: "used",
            unitType: "numeric",
            unitMeasure: "g",
            scaleBase: indicator._dataScaleBase,
          });
          const activityPercent = getDiskSpaceActivityPercent(indicator, entry.filesystem);
          const primaryStyle = indicator._getUsageColor(entry.usedPercent, indicator._diskSpaceColors);

          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            `${indicator._getValueFixed(diskSpaceDisplay.value, "diskSpace")}`,
            diskSpaceDisplay.unit,
            primaryStyle
          );
          indicator._diskSpaceBox.update_element_secondary_value(
            entry.filesystem,
            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,
            "%"
          );"#;

/// Marker proving refreshers.js already carries the current disk-space update.
const REFRESH_UPDATE_MARKER: &str = "const diskSpaceUsageDisplay = buildDiskSpaceDisplay(entry, {";

/// Shared prelude of the two upstream refresh forms; also the stale block the
/// migration removes when it precedes an already-applied usage patch.
const UPSTREAM_DISPLAY_BLOCK: &str = r#"          const display = buildDiskSpaceDisplay(entry, {
            monitor: indicator._diskSpaceMonitor,
            unitType: indicator._diskSpaceUnitType,
            unitMeasure: indicator._diskSpaceUnitMeasure,
            scaleBase: indicator._dataScaleBase,
          });

"#;

const CURRENT_UPSTREAM_UPDATE: &str = r#"          indicator._diskSpaceBox.update_element_value(
            entry.filesystem,
            display.isPercent
              ? `${display.value}`
              : `${indicator._getValueFixed(display.value)}`,
            display.unit,
            indicator._getUsageColor(display.value, indicator._diskSpaceColors)
          );"#;

const ORIGINAL_DISK_REFRESH_RESULT: &str = r#"        return {
          filesystem: device.device,
          usedBytes: Math.max(0, size - free),
          availableBytes: Math.max(0, free),
          usedPercent: size > 0 ? Math.round((100 * (size - free)) / size) : 0,
        };"#;

const FIXED_DISK_REFRESH_RESULT: &str = r#"        return {
          filesystem: device.mountPoint || device.device,
          devicePath: device.device,
          usedBytes: Math.max(0, size - free),
          availableBytes: Math.max(0, free),
          usedPercent: size > 0 ? Math.round((100 * (size - free)) / size) : 0,
        };"#;

const DISK_ROW_KEY_MARKER: &str = "devicePath: device.device,";

const ACTIVITY_HELPER_MARKER: &str =
    "function getDiskSpaceActivityPercent(indicator, filesystem)";
const COLOR_HELPER_MARKER: &str = "function getDiskUsagePercentStyle(value)";
const REFRESH_FUNCTION_MARKER: &str = "export function refreshDiskSpaceValue(indicator) {";

// ── extension.js snippets ────────────────────────────────────────────────────

const ORIGINAL_EXTENSION_DISK_LIST: &str = r#"      this._diskDevices.forEach((device) => {
        if (device.stats) {
          this._diskStatsBox.add_element(device.device, device.displayName);
        }

        if (device.space) {
          this._diskSpaceBox.add_element(device.device, device.displayName);
        }
      });"#;

const FIXED_EXTENSION_DISK_LIST: &str = r#"      this._diskDevices.forEach((device) => {
        if (device.stats) {
          this._diskStatsBox.add_element(device.device, device.displayName);
        }

        if (device.space) {
          const diskSpaceKey = device.mountPoint || device.device;
          this._diskSpaceBox.add_element(diskSpaceKey, device.displayName);
        }
      });"#;

const EXTENSION_DISK_KEY_MARKER: &str = "const diskSpaceKey = device.mountPoint || device.device;";

// ── Pure text transforms ─────────────────────────────────────────────────────

/// Ensure each disk unit label gets its style applied exactly once.
///
/// Inserts a missing `.style = style;` assignment after the unit-text line and
/// collapses duplicate assignments produced by re-running the patch.
///
/// # Parameters
/// - `content`: `containers.js` content after the main patch.
pub fn normalize_disk_container_styles(content: &str) -> String {
    let mut inserted = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(index) = rest.find(UNIT_TEXT_LINE) {
        let end = index + UNIT_TEXT_LINE.len();
        inserted.push_str(&rest[..end]);
        if !rest[end..].starts_with(UNIT_STYLE_LINE) {
            inserted.push_str(UNIT_STYLE_LINE);
        }
        rest = &rest[end..];
    }
    inserted.push_str(rest);

    let mut collapsed = String::with_capacity(inserted.len());
    let mut rest = inserted.as_str();
    while let Some(index) = rest.find(UNIT_STYLE_LINE) {
        collapsed.push_str(&rest[..index]);
        collapsed.push_str(UNIT_STYLE_LINE);
        let mut end = index;
        while rest[end..].starts_with(UNIT_STYLE_LINE) {
            end += UNIT_STYLE_LINE.len();
        }
        rest = &rest[end..];
    }
    collapsed.push_str(rest);
    collapsed
}

/// Current-body marker: LVM/mapper paths need `file_read_link` to match dm-N
/// names in `/proc/diskstats`. Older patched helpers omit this.
const ACTIVITY_HELPER_CURRENT_MARKER: &str = "GLib.file_read_link(filesystem)";

/// Replace a top-level `function name(...) { ... }` whose header matches
/// `header_marker` with `replacement` (full function text including braces).
///
/// Returns `None` when the header is absent or braces are unbalanced.
fn replace_js_function(content: &str, header_marker: &str, replacement: &str) -> Option<String> {
    let start = content.find(header_marker)?;
    let brace_rel = content[start..].find('{')?;
    let brace_start = start + brace_rel;
    let mut depth = 0usize;
    let mut end = None;
    for (i, ch) in content[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = Some(brace_start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let mut out = String::with_capacity(content.len() - (end - start) + replacement.len());
    out.push_str(&content[..start]);
    out.push_str(replacement);
    out.push_str(&content[end..]);
    Some(out)
}

/// Ensure refreshers.js defines both the disk-activity and disk-usage-colour
/// helpers, injecting whichever is missing at the appropriate anchor, and
/// upgrading a stale activity helper body when the LVM/mapper fix is absent.
///
/// # Parameters
/// - `content`: `refreshers.js` content.
pub fn ensure_disk_activity_helper(content: &str) -> String {
    let mut content = content.to_string();

    if content.contains(ACTIVITY_HELPER_MARKER)
        && !content.contains(ACTIVITY_HELPER_CURRENT_MARKER)
    {
        if let Some(upgraded) =
            replace_js_function(&content, ACTIVITY_HELPER_MARKER, DISK_ACTIVITY_HELPER_BODY)
        {
            logging::info("Upgraded refreshers.js disk activity helper (LVM/mapper diskstats)");
            println!("Upgraded refreshers.js disk activity helper (LVM/mapper diskstats)");
            content = upgraded;
        }
    }

    let has_activity = content.contains(ACTIVITY_HELPER_MARKER);
    let has_color = content.contains(COLOR_HELPER_MARKER);
    if has_activity && has_color {
        return content;
    }

    if has_activity && !has_color {
        logging::info("Patched refreshers.js disk usage color helper");
        println!("Patched refreshers.js disk usage color helper");
        return content.replacen(
            ACTIVITY_HELPER_MARKER,
            &format!("{DISK_USAGE_STYLE_HELPER}\n\n{ACTIVITY_HELPER_MARKER}"),
            1,
        );
    }

    let helper = format!("{DISK_USAGE_STYLE_HELPER}\n\n{DISK_ACTIVITY_HELPER_BODY}");
    if content.contains(REFRESH_FUNCTION_MARKER) {
        logging::info("Patched refreshers.js disk activity helper");
        println!("Patched refreshers.js disk activity helper");
        return content.replacen(
            REFRESH_FUNCTION_MARKER,
            &format!("{helper}\n\n{REFRESH_FUNCTION_MARKER}"),
            1,
        );
    }

    logging::info("Prepended refreshers.js disk activity helper");
    println!("Prepended refreshers.js disk activity helper");
    format!("{helper}\n\n{content}")
}

/// Switch the primary disk style from the generic gradient helper to the
/// dedicated `getDiskUsagePercentStyle` helper.
///
/// # Parameters
/// - `content`: `refreshers.js` content.
pub fn migrate_disk_usage_style(content: &str) -> String {
    const GENERIC: &str = "          const primaryStyle = indicator._getUsageColor(diskSpaceUsageDisplay.value, indicator._diskSpaceColors);";
    const DIRECT: &str =
        "          const primaryStyle = getDiskUsagePercentStyle(diskSpaceUsageDisplay.value);";

    if !content.contains(GENERIC) {
        return content.to_string();
    }

    logging::info("Migrated refreshers.js disk usage color style");
    println!("Migrated refreshers.js disk usage color style");
    content.replacen(GENERIC, DIRECT, 1)
}

/// Upgrade an older "muddy" disk usage gradient to the brighter
/// green -> yellow -> red gradient.
///
/// # Parameters
/// - `content`: `refreshers.js` content.
pub fn migrate_disk_usage_style_gradient(content: &str) -> String {
    const MUDDY: &str = "  const ratio = Math.max(0, Math.min(1, value / 100));\n  const red = Math.round(255 * ratio);\n  const green = Math.round(255 * (1 - ratio));";
    const BRIGHT: &str = "  const ratio = Math.max(0, Math.min(1, value / 100));\n  const red = ratio <= 0.5 ? Math.round(510 * ratio) : 255;\n  const green = ratio <= 0.5 ? 255 : Math.round(510 * (1 - ratio));";

    if !content.contains(MUDDY) {
        return content.to_string();
    }

    logging::info("Migrated refreshers.js disk usage color gradient");
    println!("Migrated refreshers.js disk usage color gradient");
    content.replacen(MUDDY, BRIGHT, 1)
}

/// Migrate the primary disk display from "used percent" to "free GB".
///
/// Switches the monitor/unit-type/unit-measure settings and recolours using
/// `usedPercent`. Each substitution is applied only when its source is present.
///
/// # Parameters
/// - `content`: `refreshers.js` content.
pub fn migrate_disk_space_primary_to_free_gb(content: &str) -> String {
    const SUBSTITUTIONS: [(&str, &str); 4] = [
        (
            "            monitor: \"used\",",
            "            monitor: \"free\",",
        ),
        (
            "            unitType: \"perc\",",
            "            unitType: \"numeric\",",
        ),
        (
            "            unitMeasure: indicator._diskSpaceUnitMeasure,",
            "            unitMeasure: \"g\",",
        ),
        (
            "          const primaryStyle = getDiskUsagePercentStyle(diskSpaceUsageDisplay.value);",
            "          const primaryStyle = getDiskUsagePercentStyle(entry.usedPercent);",
        ),
    ];

    let mut migrated = content.to_string();
    for (from, to) in SUBSTITUTIONS {
        if migrated.contains(from) {
            migrated = migrated.replacen(from, to, 1);
        }
    }

    if migrated != content {
        logging::info("Migrated refreshers.js disk primary display to free GB");
        println!("Migrated refreshers.js disk primary display to free GB");
    }

    migrated
}

/// Add a colour style to the secondary disk-activity (live IO %) label.
///
/// # Parameters
/// - `content`: `refreshers.js` content.
pub fn migrate_disk_activity_style(content: &str) -> String {
    const MARKER: &str = "const activityStyle = getDiskUsagePercentStyle(activityPercent);";
    const ACTIVITY_LINE: &str =
        "          const activityPercent = getDiskSpaceActivityPercent(indicator, entry.devicePath);";
    const SECONDARY_CALL: &str = r#"          indicator._diskSpaceBox.update_element_secondary_value(
            entry.filesystem,
            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,
            "%"
          );"#;
    const STYLED_SECONDARY_CALL: &str = r#"          indicator._diskSpaceBox.update_element_secondary_value(
            entry.filesystem,
            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,
            "%",
            activityStyle
          );"#;

    if content.contains(MARKER) {
        return content.to_string();
    }

    if !content.contains(ACTIVITY_LINE) || !content.contains(SECONDARY_CALL) {
        return content.to_string();
    }

    logging::info("Migrated refreshers.js disk activity color style");
    println!("Migrated refreshers.js disk activity color style");
    content
        .replacen(ACTIVITY_LINE, &format!("{ACTIVITY_LINE}\n          {MARKER}"), 1)
        .replacen(SECONDARY_CALL, STYLED_SECONDARY_CALL, 1)
}

/// Remove a stale disk-space display block left by earlier patch revisions that
/// would otherwise sit before the current usage patch.
///
/// # Parameters
/// - `content`: `refreshers.js` content.
pub fn remove_stale_disk_space_display_block(content: &str) -> String {
    let mut search_from = 0;
    while let Some(relative) = content[search_from..].find(UPSTREAM_DISPLAY_BLOCK) {
        let start = search_from + relative;
        let end = start + UPSTREAM_DISPLAY_BLOCK.len();
        if content[end..].starts_with(REFRESH_UPDATE_MARKER) {
            logging::info("Removed stale refreshers.js disk space display block");
            println!("Removed stale refreshers.js disk space display block");
            let mut out = String::with_capacity(content.len());
            out.push_str(&content[..start]);
            out.push_str(&content[end..]);
            return out;
        }
        search_from = start + 1;
    }
    content.to_string()
}

// ── File-level patching ──────────────────────────────────────────────────────

/// Apply the `DiskContainerSpace` rewrite to `containers.js` content.
///
/// # Parameters
/// - `content`: Current `containers.js` content.
pub fn patch_containers(content: &str) -> Result<String, PatchTargetMissing> {
    // The KB variant is the same upstream class with a different default unit
    // label, so derive it instead of duplicating the whole snippet.
    let original_kb = ORIGINAL_DISK_CONTAINER.replacen(
        r#"_createUnitLabel("GB")"#,
        r#"_createUnitLabel("KB")"#,
        1,
    );
    let snippets: Vec<&str> = vec![
        ORIGINAL_DISK_CONTAINER,
        original_kb.as_str(),
        BROKEN_PATCHED_DISK_CONTAINER,
    ];
    let migrations = [
        Migration {
            from: STYLE_DISK_UNIT_FROM,
            to: STYLE_DISK_UNIT_TO,
            all: true,
            required_marker: Some(SECONDARY_INIT_MARKER),
        },
        Migration {
            from: STYLE_DISK_SECONDARY_FROM,
            to: STYLE_DISK_SECONDARY_TO,
            all: false,
            required_marker: Some(SECONDARY_INIT_MARKER),
        },
    ];

    let (patched, _status) = replace_known_snippet_with_migration(
        content,
        &snippets,
        FIXED_DISK_CONTAINER,
        DISK_CONTAINER_MARKER,
        &migrations,
        "DiskContainerSpace",
    )?;
    Ok(normalize_disk_container_styles(&patched))
}

/// Apply every disk-space edit to `refreshers.js` content.
///
/// # Parameters
/// - `content`: Current `refreshers.js` content.
pub fn patch_refreshers(content: &str) -> Result<String, PatchTargetMissing> {
    // Both upstream forms are literal text; escaping them keeps the JavaScript
    // patcher's "patterns before snippets" precedence without hand-writing
    // regex syntax.
    let patterns = [
        Regex::new(&regex::escape(&format!(
            "{UPSTREAM_DISPLAY_BLOCK}{CURRENT_UPSTREAM_UPDATE}"
        )))
        .expect("escaped literal is a valid pattern"),
        Regex::new(&regex::escape(&format!(
            "{UPSTREAM_DISPLAY_BLOCK}{ORIGINAL_REFRESH_UPDATE}"
        )))
        .expect("escaped literal is a valid pattern"),
    ];

    let (patched, _status) = replace_known_snippet_or_patterns(
        content,
        &[
            ORIGINAL_REFRESH_UPDATE,
            BROKEN_PATCHED_REFRESH_UPDATE,
            SPACE_USED_PATCHED_REFRESH_UPDATE,
            ACTIVITY_PERCENT_FREE_GB_REFRESH_UPDATE,
            FREE_GB_PRIMARY_ACTIVITY_REFRESH_UPDATE,
            USED_GB_ACTIVITY_REFRESH_UPDATE,
        ],
        &patterns,
        FIXED_REFRESH_UPDATE,
        REFRESH_UPDATE_MARKER,
        "refreshers.js disk space update",
    )?;

    let patched = migrate_disk_usage_style(&patched);
    let patched = migrate_disk_usage_style_gradient(&patched);
    let patched = remove_stale_disk_space_display_block(&patched);
    let patched = migrate_disk_space_primary_to_free_gb(&patched);
    let patched = migrate_disk_activity_style(&patched);
    let patched = replace_known_snippet_optional(
        &patched,
        &[ORIGINAL_DISK_REFRESH_RESULT],
        FIXED_DISK_REFRESH_RESULT,
        DISK_ROW_KEY_MARKER,
        "refreshers.js disk space row key",
    );
    Ok(ensure_disk_activity_helper(&patched))
}

/// Key the extension's disk-space rows by mount point.
///
/// # Parameters
/// - `content`: Current `extension.js` content.
pub fn patch_extension(content: &str) -> Result<String, PatchTargetMissing> {
    let (patched, _status) = replace_known_snippet(
        content,
        &[ORIGINAL_EXTENSION_DISK_LIST],
        FIXED_EXTENSION_DISK_LIST,
        EXTENSION_DISK_KEY_MARKER,
        "extension.js disk space row key",
    )?;
    Ok(patched)
}

fn sibling(containers_path: &Path, relative: &[&str]) -> PathBuf {
    let mut path = containers_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("..");
    for part in relative {
        path = path.join(part);
    }
    path
}

/// CLI entry point for the disk-space patcher.
///
/// # Parameters
/// - `containers_path`: Path to the extension's `panel/containers.js`. The
///   sibling `services/refreshers.js` and `extension.js` are derived from it.
///
/// Returns `0` on success and `1` when a target file is missing or a required
/// upstream snippet cannot be located.
pub fn run(containers_path: &Path) -> i32 {
    logging::info(format!("patch-disk path={}", containers_path.display()));

    let refreshers_path = sibling(containers_path, &["services", "refreshers.js"]);
    let extension_path = sibling(containers_path, &["extension.js"]);

    // Validate and transform every required target before writing anything so a
    // mid-run failure cannot leave a half-patched extension tree.
    if !refreshers_path.exists() {
        logging::error(format!(
            "Could not find refreshers.js at: {}",
            refreshers_path.display()
        ));
        eprintln!("Could not find refreshers.js at: {}", refreshers_path.display());
        return 1;
    }

    let containers_content = match fs::read_to_string(containers_path) {
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
    let refreshers_content = match fs::read_to_string(&refreshers_path) {
        Ok(content) => content,
        Err(err) => {
            logging::error(format!(
                "Could not read {}: {err}",
                refreshers_path.display()
            ));
            eprintln!("Could not read {}: {err}", refreshers_path.display());
            return 1;
        }
    };
    let extension_content = if extension_path.exists() {
        match fs::read_to_string(&extension_path) {
            Ok(content) => Some(content),
            Err(err) => {
                logging::error(format!(
                    "Could not read {}: {err}",
                    extension_path.display()
                ));
                eprintln!("Could not read {}: {err}", extension_path.display());
                return 1;
            }
        }
    } else {
        None
    };

    let patched_containers = match patch_containers(&containers_content) {
        Ok(content) => content,
        Err(err) => {
            logging::error(err.to_string());
            eprintln!("{err}");
            return 1;
        }
    };
    let patched_refreshers = match patch_refreshers(&refreshers_content) {
        Ok(content) => content,
        Err(err) => {
            logging::error(err.to_string());
            eprintln!("{err}");
            return 1;
        }
    };
    let patched_extension = if let Some(ref content) = extension_content {
        match patch_extension(content) {
            Ok(content) => Some(content),
            Err(err) => {
                logging::error(err.to_string());
                eprintln!("{err}");
                return 1;
            }
        }
    } else {
        None
    };

    let mut any_changed = false;
    if patched_containers != containers_content {
        if let Err(err) = crate::patch_text::write_atomic(containers_path, &patched_containers) {
            logging::error(format!(
                "Could not write {}: {err}",
                containers_path.display()
            ));
            eprintln!("Could not write {}: {err}", containers_path.display());
            return 1;
        }
        any_changed = true;
    }
    if patched_refreshers != refreshers_content {
        if let Err(err) = crate::patch_text::write_atomic(&refreshers_path, &patched_refreshers) {
            logging::error(format!(
                "Could not write {}: {err}",
                refreshers_path.display()
            ));
            eprintln!("Could not write {}: {err}", refreshers_path.display());
            return 1;
        }
        any_changed = true;
    }
    if let (Some(patched), Some(original)) = (patched_extension, extension_content) {
        if patched != original {
            if let Err(err) = crate::patch_text::write_atomic(&extension_path, &patched) {
                logging::error(format!(
                    "Could not write {}: {err}",
                    extension_path.display()
                ));
                eprintln!("Could not write {}: {err}", extension_path.display());
                return 1;
            }
            any_changed = true;
        }
    }

    if any_changed {
        logging::info("Patched Resource Monitor disk free-space and live IO activity display");
        println!("Patched Resource Monitor disk free-space and live IO activity display");
    } else {
        logging::info("Disk free-space and live IO activity display already patched — skipping");
        println!("Disk free-space and live IO activity display already patched — skipping");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── containers.js ────────────────────────────────────────────────────

    #[test]
    fn containers_upstream_gb_variant_is_rewritten() {
        let patched = patch_containers(ORIGINAL_DISK_CONTAINER).expect("patch");
        assert!(patched.contains("update_element_secondary_value(filesystem, value, unit, style = \"\")"));
        assert!(patched.contains("cleanup_elements()"));
        assert!(patched.contains(DISK_CONTAINER_MARKER));
    }

    #[test]
    fn containers_upstream_kb_variant_is_rewritten() {
        let kb = ORIGINAL_DISK_CONTAINER.replacen(
            r#"_createUnitLabel("GB")"#,
            r#"_createUnitLabel("KB")"#,
            1,
        );
        let patched = patch_containers(&kb).expect("patch");
        assert!(patched.contains(DISK_CONTAINER_MARKER));
    }

    #[test]
    fn containers_patch_is_idempotent() {
        let once = patch_containers(ORIGINAL_DISK_CONTAINER).expect("first");
        let twice = patch_containers(&once).expect("second");
        assert_eq!(once, twice);
    }

    #[test]
    fn containers_missing_target_fails_fast() {
        assert!(patch_containers("// unrelated").is_err());
    }

    #[test]
    fn broken_patched_container_migrates_to_fixed_form() {
        // Older broken form already had secondary units but lacked _init /
        // cleanup and used "GB" for the secondary unit. The marker must not
        // short-circuit that migration.
        let patched = patch_containers(BROKEN_PATCHED_DISK_CONTAINER).expect("patch");
        assert!(patched.contains(DISK_CONTAINER_MARKER));
        assert!(patched.contains("cleanup_elements()"));
        assert!(patched.contains(r#"_createUnitLabel("%""#));
        assert!(!patched.contains(
            "this._elementsSecondaryUnit[filesystem] = _createUnitLabel(\"GB\""
        ));
    }

    #[test]
    fn unit_styles_are_inserted_once_and_deduplicated() {
        let missing = format!("x\n{UNIT_TEXT_LINE}y\n");
        let normalized = normalize_disk_container_styles(&missing);
        assert_eq!(normalized, format!("x\n{UNIT_TEXT_LINE}{UNIT_STYLE_LINE}y\n"));

        let duplicated = format!("x\n{UNIT_TEXT_LINE}{UNIT_STYLE_LINE}{UNIT_STYLE_LINE}y\n");
        assert_eq!(
            normalize_disk_container_styles(&duplicated),
            format!("x\n{UNIT_TEXT_LINE}{UNIT_STYLE_LINE}y\n")
        );
    }

    // ── refreshers.js ────────────────────────────────────────────────────

    fn upstream_refreshers() -> String {
        format!(
            "{REFRESH_FUNCTION_MARKER}\n{UPSTREAM_DISPLAY_BLOCK}{CURRENT_UPSTREAM_UPDATE}\n}}\n\n\
             {ORIGINAL_DISK_REFRESH_RESULT}\n"
        )
    }

    #[test]
    fn refreshers_current_upstream_form_is_patched() {
        let patched = patch_refreshers(&upstream_refreshers()).expect("patch");
        assert!(patched.contains(REFRESH_UPDATE_MARKER));
        assert!(patched.contains("const activityStyle = getDiskUsagePercentStyle(activityPercent);"));
        assert!(patched.contains(ACTIVITY_HELPER_MARKER));
        assert!(patched.contains(COLOR_HELPER_MARKER));
        assert!(patched.contains("devicePath: device.device,"));
        // The template literals must survive verbatim, not be eaten as regex
        // capture-group references.
        assert!(patched
            .contains(r#"`${indicator._getValueFixed(diskSpaceUsageDisplay.value, "diskSpace")}`"#));
        // The stale upstream display block must not be left behind.
        assert!(!patched.contains("monitor: indicator._diskSpaceMonitor,"));
    }

    #[test]
    fn refreshers_legacy_upstream_form_is_patched() {
        let legacy = format!(
            "{REFRESH_FUNCTION_MARKER}\n{UPSTREAM_DISPLAY_BLOCK}{ORIGINAL_REFRESH_UPDATE}\n}}\n"
        );
        let patched = patch_refreshers(&legacy).expect("patch");
        assert!(patched.contains(REFRESH_UPDATE_MARKER));
        assert!(!patched.contains("monitor: indicator._diskSpaceMonitor,"));
    }

    #[test]
    fn refreshers_patch_is_idempotent() {
        let once = patch_refreshers(&upstream_refreshers()).expect("first");
        let twice = patch_refreshers(&once).expect("second");
        assert_eq!(once, twice);
    }

    #[test]
    fn refreshers_older_patched_forms_converge() {
        for older in [
            BROKEN_PATCHED_REFRESH_UPDATE,
            SPACE_USED_PATCHED_REFRESH_UPDATE,
            ACTIVITY_PERCENT_FREE_GB_REFRESH_UPDATE,
            FREE_GB_PRIMARY_ACTIVITY_REFRESH_UPDATE,
            USED_GB_ACTIVITY_REFRESH_UPDATE,
        ] {
            let source = format!("{REFRESH_FUNCTION_MARKER}\n{older}\n}}\n");
            let patched = patch_refreshers(&source).expect("patch older form");
            assert!(
                patched.contains(REFRESH_UPDATE_MARKER),
                "older form was not migrated"
            );
            assert!(patched.contains(ACTIVITY_HELPER_MARKER));
        }
    }

    #[test]
    fn refreshers_missing_target_fails_fast() {
        assert!(patch_refreshers("// unrelated").is_err());
    }

    #[test]
    fn muddy_gradient_is_brightened() {
        let muddy = "function getDiskUsagePercentStyle(value) {\n  const ratio = Math.max(0, Math.min(1, value / 100));\n  const red = Math.round(255 * ratio);\n  const green = Math.round(255 * (1 - ratio));\n}";
        let migrated = migrate_disk_usage_style_gradient(muddy);
        assert!(migrated.contains("ratio <= 0.5 ? Math.round(510 * ratio) : 255"));
        assert_eq!(migrate_disk_usage_style_gradient(&migrated), migrated);
    }

    #[test]
    fn colour_helper_alone_is_injected_next_to_the_activity_helper() {
        let source = format!("{DISK_ACTIVITY_HELPER_BODY}\n");
        let patched = ensure_disk_activity_helper(&source);
        assert!(patched.contains(COLOR_HELPER_MARKER));
        assert!(patched.find(COLOR_HELPER_MARKER) < patched.find(ACTIVITY_HELPER_MARKER));
    }

    #[test]
    fn stale_activity_helper_is_upgraded_with_lvm_mapper_resolution() {
        let legacy = DISK_ACTIVITY_HELPER_BODY.replacen(
            r#"    const diskName = filesystem.replace(/^\/dev\//, "");
    const names = [diskName];
    // LVM/mapper paths show as /dev/mapper/foo while diskstats uses dm-N.
    try {
      const linkTarget = GLib.file_read_link(filesystem);
      if (linkTarget) {
        const base = String(linkTarget).replace(/^.*\//, "");
        if (base && !names.includes(base)) {
          names.push(base);
        }
      }
    } catch (_linkError) {
      // Not a symlink — keep the basename match only.
    }

    const diskStatsLine = new TextDecoder()
      .decode(diskStatsContents)
      .split("\n")
      .map((line) => line.trim().split(/\s+/))
      .find((fields) => fields.length >= 13 && names.includes(fields[2]));"#,
            r#"    const diskName = filesystem.replace(/^\/dev\//, "");
    const diskStatsLine = new TextDecoder()
      .decode(diskStatsContents)
      .split("\n")
      .map((line) => line.trim().split(/\s+/))
      .find((fields) => fields.length >= 13 && fields[2] === diskName);"#,
            1,
        );
        assert!(!legacy.contains(ACTIVITY_HELPER_CURRENT_MARKER));
        let source = format!("{DISK_USAGE_STYLE_HELPER}\n\n{legacy}\n");
        let patched = ensure_disk_activity_helper(&source);
        assert!(patched.contains(ACTIVITY_HELPER_CURRENT_MARKER));
        assert!(patched.contains("names.includes(fields[2])"));
        let twice = ensure_disk_activity_helper(&patched);
        assert_eq!(patched, twice);
    }

    #[test]
    fn helpers_are_prepended_when_no_anchor_exists() {
        let patched = ensure_disk_activity_helper("// nothing here\n");
        assert!(patched.starts_with(COLOR_HELPER_MARKER));
        assert!(patched.ends_with("// nothing here\n"));
    }

    // ── extension.js ─────────────────────────────────────────────────────

    #[test]
    fn extension_disk_rows_are_keyed_by_mount_point() {
        let patched = patch_extension(ORIGINAL_EXTENSION_DISK_LIST).expect("patch");
        assert!(patched.contains(EXTENSION_DISK_KEY_MARKER));
        let twice = patch_extension(&patched).expect("second");
        assert_eq!(patched, twice);
    }

    #[test]
    fn extension_missing_target_fails_fast() {
        assert!(patch_extension("// unrelated").is_err());
    }

    // ── End-to-end ───────────────────────────────────────────────────────

    fn extension_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("panel")).expect("panel dir");
        fs::create_dir_all(dir.path().join("services")).expect("services dir");
        fs::write(
            dir.path().join("panel/containers.js"),
            format!("{ORIGINAL_DISK_CONTAINER}\n"),
        )
        .expect("containers.js");
        fs::write(
            dir.path().join("services/refreshers.js"),
            upstream_refreshers(),
        )
        .expect("refreshers.js");
        fs::write(
            dir.path().join("extension.js"),
            format!("{ORIGINAL_EXTENSION_DISK_LIST}\n"),
        )
        .expect("extension.js");
        dir
    }

    #[test]
    fn run_patches_all_three_files_idempotently() {
        let dir = extension_tree();
        let containers = dir.path().join("panel/containers.js");
        assert_eq!(run(&containers), 0);

        let read_all = || {
            (
                fs::read_to_string(&containers).expect("containers"),
                fs::read_to_string(dir.path().join("services/refreshers.js")).expect("refreshers"),
                fs::read_to_string(dir.path().join("extension.js")).expect("extension"),
            )
        };
        let first = read_all();
        assert!(first.0.contains(DISK_CONTAINER_MARKER));
        assert!(first.1.contains(REFRESH_UPDATE_MARKER));
        assert!(first.2.contains(EXTENSION_DISK_KEY_MARKER));

        assert_eq!(run(&containers), 0);
        assert_eq!(first, read_all());
    }

    #[test]
    fn run_reports_a_missing_refreshers_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("panel")).expect("panel dir");
        let containers = dir.path().join("panel/containers.js");
        let original = format!("{ORIGINAL_DISK_CONTAINER}\n");
        fs::write(&containers, &original).expect("write");
        assert_eq!(run(&containers), 1);
        assert_eq!(
            fs::read_to_string(&containers).expect("read"),
            original,
            "containers.js must stay untouched when refreshers.js is missing"
        );
    }

    #[test]
    fn run_reports_a_missing_containers_file() {
        assert_eq!(run(Path::new("/nonexistent/panel/containers.js")), 1);
    }
}
