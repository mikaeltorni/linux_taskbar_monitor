// patch_resource_monitor_disk.js - Patch Resource Monitor disk space display.
//
// Components:
//   - DiskContainerSpace patch: adds a secondary label for live disk activity.
//   - extension.js patch: keys disk space rows by mount point.
//   - getDiskSpaceActivityPercent helper: calculates per-refresh disk IO busy percentage.
//   - refreshers.js patch: renders free GB plus live IO busy percent for each disk.
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

function replaceKnownSnippetOrPattern(
  content,
  snippets,
  pattern,
  replacement,
  alreadyMarker,
  targetName
) {
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

  if (pattern.test(content)) {
    console.log(`Patched ${targetName}`);
    return content.replace(pattern, replacement);
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

    update_element_secondary_value(filesystem, value, unit) {
      if (this._elementsSecondaryValue[filesystem]) {
        this._elementsSecondaryValue[filesystem].text = value;
        this._elementsSecondaryUnit[filesystem].text = unit;
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
      }
    }

    update_element_secondary_value(filesystem, value, unit) {
      if (this._elementsSecondaryValue[filesystem]) {
        this._elementsSecondaryValue[filesystem].text = value;
        this._elementsSecondaryUnit[filesystem].text = unit;
      }
    }
  }
);`;

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

const diskActivityHelper = [
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

const fixedRefreshUpdate = [
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
  const marker = "function getDiskSpaceActivityPercent(indicator, filesystem)";
  if (content.includes(marker)) {
    return content;
  }

  const refreshFunctionMarker = "export function refreshDiskSpaceValue(indicator) {";
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

let content = fs.readFileSync(containersPath, "utf8");
content = replaceKnownSnippet(
  content,
  [originalDiskContainer, originalDiskContainerKb, brokenPatchedDiskContainer],
  fixedDiskContainer,
  "this._elementsSecondaryValue = [];",
  "DiskContainerSpace"
);
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
refreshersContent = replaceKnownSnippetOrPattern(
  refreshersContent,
  [
    originalRefreshUpdate,
    brokenPatchedRefreshUpdate,
    spaceUsedPatchedRefreshUpdate,
    activityPercentFreeGbRefreshUpdate,
    usedGbActivityRefreshUpdate,
  ],
  currentUpstreamRefreshPattern,
  fixedRefreshUpdate,
  "const activityPercent = getDiskSpaceActivityPercent(indicator, entry.devicePath);",
  "refreshers.js disk space update"
);
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
