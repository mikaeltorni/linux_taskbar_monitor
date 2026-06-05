// patch_resource_monitor_disk.js - Patch Resource Monitor disk space display.
//
// Components:
//   - DiskContainerSpace patch: adds a secondary label for live disk activity.
//   - extension.js patch: keys disk space rows by mount point.
//   - getDiskSpaceActivityPercent helper: calculates per-refresh disk IO busy percentage.
//   - refreshers.js patch: renders used percentage plus live IO busy percent for each disk.
//
// Usage:
//   node scripts/patch_resource_monitor_disk.js <path-to-containers.js>

const fs = require("fs");
const path = require("path");

const containersPath = process.argv[2];
if (!containersPath) {
  console.error("Usage: patch_resource_monitor_disk.js <path-to-containers.js>");
  process.exit(1);
}

function replaceKnownSnippet(content, snippets, replacement, alreadyMarker, targetName) {
  if (content.includes(alreadyMarker)) {
    console.log(`${targetName} already patched`);
    return content;
  }

  for (const snippet of snippets) {
    if (content.includes(snippet)) {
      console.log(`Patched ${targetName}`);
      return content.replace(snippet, replacement);
    }
  }

  console.error(`Could not find target code in ${targetName}`);
  process.exit(1);
}

function replaceKnownSnippetWithMigration(
  content,
  snippets,
  replacement,
  alreadyMarker,
  migrations,
  targetName
) {
  let migratedContent = content;
  let didMigrate = false;
  for (const migration of migrations) {
    const requiredMarker = migration.requiredMarker || "";
    if (
      (!requiredMarker || migratedContent.includes(requiredMarker)) &&
      migratedContent.includes(migration.from)
    ) {
      migratedContent = migration.all
        ? migratedContent.split(migration.from).join(migration.to)
        : migratedContent.replace(migration.from, migration.to);
      didMigrate = true;
    }
  }

  if (didMigrate) {
    console.log(`Migrated ${targetName}`);
    return migratedContent;
  }

  if (content.includes(alreadyMarker)) {
    console.log(`${targetName} already patched`);
    return content;
  }

  return replaceKnownSnippet(content, snippets, replacement, alreadyMarker, targetName);
}

function replaceKnownSnippetOrPatterns(
  content,
  snippets,
  patterns,
  replacement,
  alreadyMarker,
  targetName
) {
  if (content.includes(alreadyMarker)) {
    console.log(`${targetName} already patched`);
    return content;
  }

  for (const pattern of patterns) {
    if (pattern.test(content)) {
      console.log(`Patched ${targetName}`);
      return content.replace(pattern, replacement);
    }
  }

  for (const snippet of snippets) {
    if (content.includes(snippet)) {
      console.log(`Patched ${targetName}`);
      return content.replace(snippet, replacement);
    }
  }

  console.error(`Could not find target code in ${targetName}`);
  process.exit(1);
}

function replaceKnownSnippetOptional(content, snippets, replacement, alreadyMarker, targetName) {
  if (content.includes(alreadyMarker)) {
    console.log(`${targetName} already patched`);
    return content;
  }

  for (const snippet of snippets) {
    if (content.includes(snippet)) {
      console.log(`Patched ${targetName}`);
      return content.replace(snippet, replacement);
    }
  }

  console.log(`${targetName} target not found; skipping`);
  return content;
}

function normalizeDiskContainerStyles(content) {
  return content
    .replace(
      /(        this\._elementsUnit\[filesystem\]\.text = unit;\n)(?!        this\._elementsUnit\[filesystem\]\.style = style;\n)/g,
      "$1        this._elementsUnit[filesystem].style = style;\n"
    )
    .replace(
      /(        this\._elementsUnit\[filesystem\]\.style = style;\n){2,}/g,
      "        this._elementsUnit[filesystem].style = style;\n"
    );
}

function getDiskUsagePercentStyle(value) {
  if (!Number.isFinite(value)) {
    return "";
  }

  const ratio = Math.max(0, Math.min(1, value / 100));
  const red = ratio <= 0.5 ? Math.round(510 * ratio) : 255;
  const green = ratio <= 0.5 ? 255 : Math.round(510 * (1 - ratio));

  return `color: rgb(${red}, ${green}, 0);`;
}

const originalDiskContainer = `export const DiskContainerSpace = GObject.registerClass(
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
);`;

const originalDiskContainerKb = originalDiskContainer.replace(
  '_createUnitLabel("GB")',
  '_createUnitLabel("KB")'
);

const brokenPatchedDiskContainer = `export const DiskContainerSpace = GObject.registerClass(
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
);`;

const fixedDiskContainer = `export const DiskContainerSpace = GObject.registerClass(
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
);`;

const styleDiskUnitMigration = {
  requiredMarker: "this._elementsSecondaryValue = [];",
  all: true,
  from: [
    "        this._elementsValue[filesystem].text = value;",
    "        this._elementsValue[filesystem].style = style;",
    "        this._elementsUnit[filesystem].text = unit;",
  ].join("\n"),
  to: [
    "        this._elementsValue[filesystem].text = value;",
    "        this._elementsValue[filesystem].style = style;",
    "        this._elementsUnit[filesystem].text = unit;",
    "        this._elementsUnit[filesystem].style = style;",
  ].join("\n"),
};

const styleDiskSecondaryMigration = {
  requiredMarker: "this._elementsSecondaryValue = [];",
  from: [
    "    update_element_secondary_value(filesystem, value, unit) {",
    "      if (this._elementsSecondaryValue[filesystem]) {",
    "        this._elementsSecondaryValue[filesystem].text = value;",
    "        this._elementsSecondaryUnit[filesystem].text = unit;",
    "      }",
    "    }",
  ].join("\n"),
  to: [
    '    update_element_secondary_value(filesystem, value, unit, style = "") {',
    "      if (this._elementsSecondaryValue[filesystem]) {",
    "        this._elementsSecondaryValue[filesystem].text = value;",
    "        this._elementsSecondaryValue[filesystem].style = style;",
    "        this._elementsSecondaryUnit[filesystem].text = unit;",
    "        this._elementsSecondaryUnit[filesystem].style = style;",
    "      }",
    "    }",
  ].join("\n"),
};

const originalRefreshUpdate = [
  "          indicator._diskSpaceBox.update_element_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(display.value, "diskSpace")}`,',
  "            display.unit,",
  "            indicator._getUsageColor(display.value, indicator._diskSpaceColors)",
  "          );",
].join("\n");

const brokenPatchedRefreshUpdate = [
  "          // Primary: used percentage (always shown)",
  "          const primaryValue = entry.usedPercent;",
  '          const primaryUnit = "%";',
  "          const primaryStyle = indicator._getUsageColor(primaryValue, indicator._diskSpaceColors);",
  "",
  "          // Secondary: free space in GB (numeric display)",
  '          const factor = getDataScaleFactor(indicator._dataScaleBase || "decimal");',
  "          const freeInGb = entry.availableBytes / (factor * 1024);",
  "          const secondaryValue = Math.round(freeInGb * 10) / 10;",
  '          const secondaryUnit = "GB";',
  "",
  "          indicator._diskSpaceBox.update_element_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(primaryValue, "diskSpace")}`,',
  "            primaryUnit,",
  "            primaryStyle",
  "          );",
  "          indicator._diskSpaceBox.update_element_secondary_value(",
  "            entry.filesystem,",
  "            secondaryValue.toFixed(1),",
  "            secondaryUnit",
  "          );",
].join("\n");

const spaceUsedPatchedRefreshUpdate = [
  "          const primaryDisplay = buildDiskSpaceDisplay(entry, {",
  '            monitor: "used",',
  '            unitType: "perc",',
  "            unitMeasure: indicator._diskSpaceUnitMeasure,",
  "            scaleBase: indicator._dataScaleBase,",
  "          });",
  "          const freeDisplay = buildDiskSpaceDisplay(entry, {",
  '            monitor: "free",',
  '            unitType: "numeric",',
  '            unitMeasure: "g",',
  "            scaleBase: indicator._dataScaleBase,",
  "          });",
  "          const primaryStyle = indicator._getUsageColor(",
  "            entry.usedPercent,",
  "            indicator._diskSpaceColors",
  "          );",
  "",
  "          indicator._diskSpaceBox.update_element_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(primaryDisplay.value, "diskSpace")}`,',
  "            primaryDisplay.unit,",
  "            primaryStyle",
  "          );",
  "          indicator._diskSpaceBox.update_element_secondary_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(freeDisplay.value, "diskSpace")}`,',
  "            freeDisplay.unit",
  "          );",
].join("\n");

const diskUsageStyleHelper = [
  "function getDiskUsagePercentStyle(value) {",
  "  if (!Number.isFinite(value)) {",
  '    return "";',
  "  }",
  "",
  "  const ratio = Math.max(0, Math.min(1, value / 100));",
  "  const red = ratio <= 0.5 ? Math.round(510 * ratio) : 255;",
  "  const green = ratio <= 0.5 ? 255 : Math.round(510 * (1 - ratio));",
  "",
  "  return `color: rgb(${red}, ${green}, 0);`;",
  "}",
].join("\n");

const diskActivityHelper = [
  diskUsageStyleHelper,
  "",
  "function getDiskSpaceActivityPercent(indicator, filesystem) {",
  "  if (!indicator._diskSpaceActivitySamples) {",
  "    indicator._diskSpaceActivitySamples = new Map();",
  "  }",
  "",
  "  try {",
  '    const [ok, diskStatsContents] = GLib.file_get_contents("/proc/diskstats");',
  "    if (!ok) {",
  "      return 0;",
  "    }",
  "",
  '    const diskName = filesystem.replace(/^\\/dev\\//, "");',
  "    const diskStatsLine = new TextDecoder()",
  "      .decode(diskStatsContents)",
  '      .split("\\n")',
  "      .map((line) => line.trim().split(/\\s+/))",
  "      .find((fields) => fields.length >= 13 && fields[2] === diskName);",
  "",
  "    if (!diskStatsLine) {",
  "      return 0;",
  "    }",
  "",
  "    const ioTimeMs = parseInt(diskStatsLine[12], 10);",
  "    if (!Number.isFinite(ioTimeMs)) {",
  "      return 0;",
  "    }",
  "",
  "    const sampleTimeMs = GLib.get_monotonic_time() / 1000;",
  "    const previousSample = indicator._diskSpaceActivitySamples.get(filesystem);",
  "",
  "    indicator._diskSpaceActivitySamples.set(filesystem, {",
  "      ioTimeMs,",
  "      sampleTimeMs,",
  "    });",
  "",
  "    if (!previousSample || !Number.isFinite(previousSample.ioTimeMs)) {",
  "      return 0;",
  "    }",
  "",
  "    const elapsedMs = Math.max(0, sampleTimeMs - previousSample.sampleTimeMs);",
  "    const activeMs = Math.max(0, ioTimeMs - previousSample.ioTimeMs);",
  "",
  "    return elapsedMs > 0 ? Math.min(100, (activeMs * 100) / elapsedMs) : 0;",
  "  } catch (error) {",
  "    return 0;",
  "  }",
  "}",
].join("\n");

const activityPercentFreeGbRefreshUpdate = [
  "          if (!indicator._diskSpaceActivitySamples) {",
  "            indicator._diskSpaceActivitySamples = new Map();",
  "          }",
  "",
  "          let activityPercent = 0;",
  "          try {",
  '            const [ok, diskStatsContents] = GLib.file_get_contents("/proc/diskstats");',
  "            if (ok) {",
  '              const diskName = entry.filesystem.replace(/^\\/dev\\//, "");',
  "              const diskStatsLine = new TextDecoder()",
  "                .decode(diskStatsContents)",
  '                .split("\\n")',
  "                .map((line) => line.trim().split(/\\s+/))",
  "                .find((fields) => fields.length >= 13 && fields[2] === diskName);",
  "",
  "              if (diskStatsLine) {",
  "                const ioTimeMs = parseInt(diskStatsLine[12], 10);",
  "                const sampleTimeMs = GLib.get_monotonic_time() / 1000;",
  "                const previousSample = indicator._diskSpaceActivitySamples.get(",
  "                  entry.filesystem",
  "                );",
  "",
  "                indicator._diskSpaceActivitySamples.set(entry.filesystem, {",
  "                  ioTimeMs,",
  "                  sampleTimeMs,",
  "                });",
  "",
  "                if (previousSample && Number.isFinite(ioTimeMs)) {",
  "                  const elapsedMs = Math.max(",
  "                    0,",
  "                    sampleTimeMs - previousSample.sampleTimeMs",
  "                  );",
  "                  const activeMs = Math.max(",
  "                    0,",
  "                    ioTimeMs - previousSample.ioTimeMs",
  "                  );",
  "                  activityPercent = elapsedMs > 0",
  "                    ? Math.min(100, (activeMs * 100) / elapsedMs)",
  "                    : 0;",
  "                }",
  "              }",
  "            }",
  "          } catch (error) {",
  "            activityPercent = 0;",
  "          }",
  "",
  "          const freeDisplay = buildDiskSpaceDisplay(entry, {",
  '            monitor: "free",',
  '            unitType: "numeric",',
  '            unitMeasure: "g",',
  "            scaleBase: indicator._dataScaleBase,",
  "          });",
  "          const primaryStyle = indicator._getUsageColor(",
  "            activityPercent,",
  "            indicator._diskSpaceColors",
  "          );",
  "",
  "          indicator._diskSpaceBox.update_element_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,',
  '            "%",',
  "            primaryStyle",
  "          );",
  "          indicator._diskSpaceBox.update_element_secondary_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(freeDisplay.value, "diskSpace")}`,',
  "            freeDisplay.unit",
  "          );",
].join("\n");

const freeGbPrimaryActivityRefreshUpdate = [
  "          const diskSpaceDisplay = buildDiskSpaceDisplay(entry, {",
  '            monitor: "free",',
  '            unitType: "numeric",',
  '            unitMeasure: "g",',
  "            scaleBase: indicator._dataScaleBase,",
  "          });",
  "          const activityPercent = getDiskSpaceActivityPercent(indicator, entry.devicePath);",
  "          const primaryStyle = indicator._getUsageColor(entry.usedPercent, indicator._diskSpaceColors);",
  "",
  "          indicator._diskSpaceBox.update_element_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(diskSpaceDisplay.value, "diskSpace")}`,',
  "            diskSpaceDisplay.unit,",
  "            primaryStyle",
  "          );",
  "          indicator._diskSpaceBox.update_element_secondary_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,',
  '            "%"',
  "          );",
].join("\n");

const fixedRefreshUpdate = [
  "          const diskSpaceUsageDisplay = buildDiskSpaceDisplay(entry, {",
  '            monitor: "used",',
  '            unitType: "numeric",',
  '            unitMeasure: "g",',
  "            scaleBase: indicator._dataScaleBase,",
  "          });",
  "          const activityPercent = getDiskSpaceActivityPercent(indicator, entry.devicePath);",
  "          const primaryStyle = getDiskUsagePercentStyle(entry.usedPercent);",
  "          const activityStyle = getDiskUsagePercentStyle(activityPercent);",
  "",
  "          indicator._diskSpaceBox.update_element_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(diskSpaceUsageDisplay.value, "diskSpace")}`,',
  "            diskSpaceUsageDisplay.unit,",
  "            primaryStyle",
  "          );",
  "          indicator._diskSpaceBox.update_element_secondary_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,',
  '            "%",',
  "            activityStyle",
  "          );",
].join("\n");

const usedGbActivityRefreshUpdate = [
  "          const diskSpaceDisplay = buildDiskSpaceDisplay(entry, {",
  '            monitor: "used",',
  '            unitType: "numeric",',
  '            unitMeasure: "g",',
  "            scaleBase: indicator._dataScaleBase,",
  "          });",
  "          const activityPercent = getDiskSpaceActivityPercent(indicator, entry.filesystem);",
  "          const primaryStyle = indicator._getUsageColor(entry.usedPercent, indicator._diskSpaceColors);",
  "",
  "          indicator._diskSpaceBox.update_element_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(diskSpaceDisplay.value, "diskSpace")}`,',
  "            diskSpaceDisplay.unit,",
  "            primaryStyle",
  "          );",
  "          indicator._diskSpaceBox.update_element_secondary_value(",
  "            entry.filesystem,",
  '            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,',
  '            "%"',
  "          );",
].join("\n");

const currentUpstreamRefreshPattern =
  /          const display = buildDiskSpaceDisplay\(entry, \{\n            monitor: indicator\._diskSpaceMonitor,\n            unitType: indicator\._diskSpaceUnitType,\n            unitMeasure: indicator\._diskSpaceUnitMeasure,\n            scaleBase: indicator\._dataScaleBase,\n          \}\);\n\n          indicator\._diskSpaceBox\.update_element_value\(\n            entry\.filesystem,\n            display\.isPercent\n              \? `\$\{display\.value\}`\n              : `\$\{indicator\._getValueFixed\(display\.value\)\}`,\n            display\.unit,\n            indicator\._getUsageColor\(display\.value, indicator\._diskSpaceColors\)\n          \);/;

const legacyUpstreamRefreshPattern =
  /          const display = buildDiskSpaceDisplay\(entry, \{\n            monitor: indicator\._diskSpaceMonitor,\n            unitType: indicator\._diskSpaceUnitType,\n            unitMeasure: indicator\._diskSpaceUnitMeasure,\n            scaleBase: indicator\._dataScaleBase,\n          \}\);\n\n          indicator\._diskSpaceBox\.update_element_value\(\n            entry\.filesystem,\n            `\$\{indicator\._getValueFixed\(display\.value, "diskSpace"\)\}`,\n            display\.unit,\n            indicator\._getUsageColor\(display\.value, indicator\._diskSpaceColors\)\n          \);/;

const staleDisplayBlockBeforeUsagePatchPattern =
  /          const display = buildDiskSpaceDisplay\(entry, \{\n            monitor: indicator\._diskSpaceMonitor,\n            unitType: indicator\._diskSpaceUnitType,\n            unitMeasure: indicator\._diskSpaceUnitMeasure,\n            scaleBase: indicator\._dataScaleBase,\n          \}\);\n\n(?=          const diskSpaceUsageDisplay = buildDiskSpaceDisplay\(entry, \{)/;

const originalDiskRefreshResult = [
  "        return {",
  "          filesystem: device.device,",
  "          usedBytes: Math.max(0, size - free),",
  "          availableBytes: Math.max(0, free),",
  "          usedPercent: size > 0 ? Math.round((100 * (size - free)) / size) : 0,",
  "        };",
].join("\n");

const fixedDiskRefreshResult = [
  "        return {",
  "          filesystem: device.mountPoint || device.device,",
  "          devicePath: device.device,",
  "          usedBytes: Math.max(0, size - free),",
  "          availableBytes: Math.max(0, free),",
  "          usedPercent: size > 0 ? Math.round((100 * (size - free)) / size) : 0,",
  "        };",
].join("\n");

const originalExtensionDiskList = [
  "      this._diskDevices.forEach((device) => {",
  "        if (device.stats) {",
  "          this._diskStatsBox.add_element(device.device, device.displayName);",
  "        }",
  "",
  "        if (device.space) {",
  "          this._diskSpaceBox.add_element(device.device, device.displayName);",
  "        }",
  "      });",
].join("\n");

const fixedExtensionDiskList = [
  "      this._diskDevices.forEach((device) => {",
  "        if (device.stats) {",
  "          this._diskStatsBox.add_element(device.device, device.displayName);",
  "        }",
  "",
  "        if (device.space) {",
  "          const diskSpaceKey = device.mountPoint || device.device;",
  "          this._diskSpaceBox.add_element(diskSpaceKey, device.displayName);",
  "        }",
  "      });",
].join("\n");

function ensureDiskActivityHelper(content) {
  const activityMarker = "function getDiskSpaceActivityPercent(indicator, filesystem)";
  const colorMarker = "function getDiskUsagePercentStyle(value)";
  if (content.includes(activityMarker) && content.includes(colorMarker)) {
    return content;
  }

  const refreshFunctionMarker = "export function refreshDiskSpaceValue(indicator) {";
  if (content.includes(activityMarker) && !content.includes(colorMarker)) {
    console.log("Patched refreshers.js disk usage color helper");
    return content.replace(activityMarker, `${diskUsageStyleHelper}\n\n${activityMarker}`);
  }

  if (content.includes(refreshFunctionMarker)) {
    console.log("Patched refreshers.js disk activity helper");
    return content.replace(
      refreshFunctionMarker,
      `${diskActivityHelper}\n\n${refreshFunctionMarker}`
    );
  }

  console.log("Prepended refreshers.js disk activity helper");
  return `${diskActivityHelper}\n\n${content}`;
}

function migrateDiskUsageStyle(content) {
  const genericStyleLine =
    "          const primaryStyle = indicator._getUsageColor(diskSpaceUsageDisplay.value, indicator._diskSpaceColors);";
  const directStyleLine =
    "          const primaryStyle = getDiskUsagePercentStyle(diskSpaceUsageDisplay.value);";

  if (!content.includes(genericStyleLine)) {
    return content;
  }

  console.log("Migrated refreshers.js disk usage color style");
  return content.replace(genericStyleLine, directStyleLine);
}

function migrateDiskUsageStyleGradient(content) {
  const muddyGradient = [
    "  const ratio = Math.max(0, Math.min(1, value / 100));",
    "  const red = Math.round(255 * ratio);",
    "  const green = Math.round(255 * (1 - ratio));",
  ].join("\n");
  const brightGradient = [
    "  const ratio = Math.max(0, Math.min(1, value / 100));",
    "  const red = ratio <= 0.5 ? Math.round(510 * ratio) : 255;",
    "  const green = ratio <= 0.5 ? 255 : Math.round(510 * (1 - ratio));",
  ].join("\n");

  if (!content.includes(muddyGradient)) {
    return content;
  }

  console.log("Migrated refreshers.js disk usage color gradient");
  return content.replace(muddyGradient, brightGradient);
}

function migrateDiskSpacePrimaryToGb(content) {
  let migratedContent = content;

  const percentUnitType = '            unitType: "perc",';
  const numericUnitType = '            unitType: "numeric",';
  if (migratedContent.includes(percentUnitType)) {
    migratedContent = migratedContent.replace(percentUnitType, numericUnitType);
  }

  const settingsUnitMeasure = "            unitMeasure: indicator._diskSpaceUnitMeasure,";
  const gbUnitMeasure = '            unitMeasure: "g",';
  if (migratedContent.includes(settingsUnitMeasure)) {
    migratedContent = migratedContent.replace(settingsUnitMeasure, gbUnitMeasure);
  }

  const displayedValueStyle =
    "          const primaryStyle = getDiskUsagePercentStyle(diskSpaceUsageDisplay.value);";
  const usedPercentStyle =
    "          const primaryStyle = getDiskUsagePercentStyle(entry.usedPercent);";
  if (migratedContent.includes(displayedValueStyle)) {
    migratedContent = migratedContent.replace(displayedValueStyle, usedPercentStyle);
  }

  if (migratedContent !== content) {
    console.log("Migrated refreshers.js disk primary display to used GB");
  }

  return migratedContent;
}

function migrateDiskActivityStyle(content) {
  const marker = "const activityStyle = getDiskUsagePercentStyle(activityPercent);";
  if (content.includes(marker)) {
    return content;
  }

  const activityLine =
    "          const activityPercent = getDiskSpaceActivityPercent(indicator, entry.devicePath);";
  const secondaryCall = [
    "          indicator._diskSpaceBox.update_element_secondary_value(",
    "            entry.filesystem,",
    '            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,',
    '            "%"',
    "          );",
  ].join("\n");
  const styledSecondaryCall = [
    "          indicator._diskSpaceBox.update_element_secondary_value(",
    "            entry.filesystem,",
    '            `${indicator._getValueFixed(activityPercent, "diskSpace")}`,',
    '            "%",',
    "            activityStyle",
    "          );",
  ].join("\n");

  if (!content.includes(activityLine) || !content.includes(secondaryCall)) {
    return content;
  }

  console.log("Migrated refreshers.js disk activity color style");
  return content
    .replace(activityLine, `${activityLine}\n          ${marker}`)
    .replace(secondaryCall, styledSecondaryCall);
}

function removeStaleDiskSpaceDisplayBlock(content) {
  if (!staleDisplayBlockBeforeUsagePatchPattern.test(content)) {
    return content;
  }

  console.log("Removed stale refreshers.js disk space display block");
  return content.replace(staleDisplayBlockBeforeUsagePatchPattern, "");
}

let content = fs.readFileSync(containersPath, "utf8");
content = replaceKnownSnippetWithMigration(
  content,
  [originalDiskContainer, originalDiskContainerKb, brokenPatchedDiskContainer],
  fixedDiskContainer,
  "this._elementsSecondaryUnit[filesystem].style = style;",
  [styleDiskUnitMigration, styleDiskSecondaryMigration],
  "DiskContainerSpace"
);
content = normalizeDiskContainerStyles(content);
fs.writeFileSync(containersPath, content);

const refreshersPath = path.join(
  path.dirname(containersPath),
  "..",
  "services",
  "refreshers.js"
);
if (!fs.existsSync(refreshersPath)) {
  console.error("Could not find refreshers.js at:", refreshersPath);
  process.exit(1);
}

let refreshersContent = fs.readFileSync(refreshersPath, "utf8");
refreshersContent = replaceKnownSnippetOrPatterns(
  refreshersContent,
  [
    originalRefreshUpdate,
    brokenPatchedRefreshUpdate,
    spaceUsedPatchedRefreshUpdate,
    activityPercentFreeGbRefreshUpdate,
    freeGbPrimaryActivityRefreshUpdate,
    usedGbActivityRefreshUpdate,
  ],
  [currentUpstreamRefreshPattern, legacyUpstreamRefreshPattern],
  fixedRefreshUpdate,
  "const diskSpaceUsageDisplay = buildDiskSpaceDisplay(entry, {",
  "refreshers.js disk space update"
);
refreshersContent = migrateDiskUsageStyle(refreshersContent);
refreshersContent = migrateDiskUsageStyleGradient(refreshersContent);
refreshersContent = removeStaleDiskSpaceDisplayBlock(refreshersContent);
refreshersContent = migrateDiskSpacePrimaryToGb(refreshersContent);
refreshersContent = migrateDiskActivityStyle(refreshersContent);
refreshersContent = replaceKnownSnippetOptional(
  refreshersContent,
  [originalDiskRefreshResult],
  fixedDiskRefreshResult,
  "filesystem: device.mountPoint || device.device",
  "refreshers.js disk space row key"
);
refreshersContent = ensureDiskActivityHelper(refreshersContent);
fs.writeFileSync(refreshersPath, refreshersContent);

const extensionPath = path.join(path.dirname(containersPath), "..", "extension.js");
if (fs.existsSync(extensionPath)) {
  let extensionContent = fs.readFileSync(extensionPath, "utf8");
  extensionContent = replaceKnownSnippet(
    extensionContent,
    [originalExtensionDiskList],
    fixedExtensionDiskList,
    "const diskSpaceKey = device.mountPoint || device.device;",
    "extension.js disk space row key"
  );
  fs.writeFileSync(extensionPath, extensionContent);
}
