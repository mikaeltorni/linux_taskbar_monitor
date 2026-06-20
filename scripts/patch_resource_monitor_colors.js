// patch_resource_monitor_colors.js — Override _getUsageColor with 256-step RGB gradients.
//
// This script patches extension.js to:
//   1. Inject gradient configuration into each indicator's color settings during init.
//      Each colors array gets a type marker (e.g., "__cpu") appended so the gradient
//      function can reliably identify which metric is being colored.
//   2. Override _getUsageColor with a gradient-based implementation that uses those
//      markers to select the correct color gradient for each indicator type.
//
// Gradient scheme:
//   - CPU:        0% green → ~50% yellow → 100% red (percentage-based)
//   - RAM:        0GB green → maxRAM(64GB) red (configurable via GSettings)
//   - Disk Space: 0% used green → 100% used red
//   - Ethernet:   0 Mb/s green → ETHERNET_MAX_MBPS Mb/s red for both upload & download
//   - Wi-Fi:      same as Ethernet
//   - GPU Usage:  0% green → 100% red (percentage-based, like CPU)
//   - GPU Memory: 0GB green → maxVRAM(nvidia-smi) red (configurable via GSettings)
//
// Usage:
//   node scripts/patch_resource_monitor_colors.js <path-to-extension.js>

const fs = require("fs");
// Gradient ranges are sourced from the tested gradient_colors module so the
// values interpolated into the injected support block have a single definition.
const {
  ETHERNET_MAX_MBPS,
  RAM_MAX_GB,
  DISK_USAGE_MAX_PERCENT,
  GPU_MEMORY_MAX_GB,
} = require("./lib/gradient_colors");

const extPath = process.argv[2];
if (!extPath) {
  console.error("Usage: patch_resource_monitor_colors.js <path-to-extension.js>");
  process.exit(1);
}

// The gradient color math lives in scripts/lib/gradient_colors.js (with tests).
// The patcher injects a self-contained copy into extension.js below, since GNOME
// Shell (GJS) cannot require Node modules at runtime.

// ── Main patching function ───────────────────────────────────────────────────

/**
 * Upgrade a previously patched extension.js from the older two-stop gradient to
 * the green → yellow → red midpoint gradient.
 *
 * Injects the getGreenYellowRedGradientColor helper when it is missing and
 * rewrites the _getUsageColor return statement to call it. Each substitution is
 * applied only when its source form is present, so the function is idempotent
 * and a no-op on content that is already current.
 * @param {string} content - Current extension.js content.
 * @returns {string} Migrated content, or unchanged content when already current.
 */
function migrateGreenYellowRedGradient(content) {
  let migratedContent = content;

  if (!migratedContent.includes("function getGreenYellowRedGradientColor")) {
    const helperInsertionPoint = [
      "  return rgbToStyle(r, g, b);",
      "}",
    ].join("\n");
    const helperBlock = [
      "  return rgbToStyle(r, g, b);",
      "}",
      "",
      "function getGreenYellowRedGradientColor(value, minVal, maxVal, startRGB, endRGB) {",
      "  const midpoint = minVal + (maxVal - minVal) / 2;",
      "  const midpointRGB = [255, 255, 0];",
      "",
      "  if (value <= midpoint) {",
      "    return getGradientColor(value, minVal, midpoint, startRGB, midpointRGB);",
      "  }",
      "",
      "  return getGradientColor(value, midpoint, maxVal, midpointRGB, endRGB);",
      "}",
    ].join("\n");

    if (migratedContent.includes(helperInsertionPoint)) {
      migratedContent = migratedContent.replace(helperInsertionPoint, helperBlock);
    }
  }

  const oldReturn =
    "      return getGradientColor(effectiveValue, minVal, maxVal, config.startRGB, config.endRGB);";
  const newReturn = [
    "      return getGreenYellowRedGradientColor(",
    "        effectiveValue,",
    "        minVal,",
    "        maxVal,",
    "        config.startRGB,",
    "        config.endRGB",
    "      );",
  ].join("\n");

  if (migratedContent.includes(oldReturn)) {
    migratedContent = migratedContent.replace(oldReturn, newReturn);
  }

  if (migratedContent !== content) {
    console.log("Migrated gradient colors to green-yellow-red midpoint");
  }

  return migratedContent;
}

/**
 * Patch extension.js to override _getUsageColor with gradient-based coloring.
 * Also injects type markers into each indicator's color settings during init.
 * @param {string} content - Original file content.
 * @returns {string} Patched file content.
 */
function patchExtensionJS(content) {
  // Check if already patched (idempotent).
  const alreadyPatchedMarker = "_gradientGetUsageColor";
  if (content.includes(alreadyPatchedMarker)) {
    content = migrateGreenYellowRedGradient(content);
    if (!content.includes("colors === this._diskSpaceColors")) {
      console.log("Migrating gradient color detection to property identity checks");
      return content
        .replace(
          "      if (colorStr.includes(\"__eth\")) {\n        config = GRADIENT_CONFIGS.eth;\n      } else if (colorStr.includes(\"__wlan\")) {\n        config = GRADIENT_CONFIGS.wlan;\n      } else if (colorStr.includes(\"__gpuMem\")) {\n        config = GRADIENT_CONFIGS.gpuMemory;\n      } else if (colorStr.includes(\"__diskSpace\")) {\n        config = GRADIENT_CONFIGS.diskSpace;\n      } else if (colorStr.includes(\"__gpu\")) {\n        config = GRADIENT_CONFIGS.gpu;\n      } else if (colorStr.includes(\"__ram\")) {\n        config = GRADIENT_CONFIGS.ram;\n      } else if (colorStr.includes(\"__cpu\")) {\n        config = GRADIENT_CONFIGS.cpu;",
          "      if (colors === this._diskSpaceColors) {\n        config = GRADIENT_CONFIGS.diskSpace;\n      } else if (colors === this._netEthColors || colorStr.includes(\"__eth\")) {\n        config = GRADIENT_CONFIGS.eth;\n      } else if (colors === this._netWlanColors || colorStr.includes(\"__wlan\")) {\n        config = GRADIENT_CONFIGS.wlan;\n      } else if (colors === this._gpuMemoryColors || colorStr.includes(\"__gpuMem\")) {\n        config = GRADIENT_CONFIGS.gpuMemory;\n      } else if (colorStr.includes(\"__diskSpace\")) {\n        config = GRADIENT_CONFIGS.diskSpace;\n      } else if (colors === this._gpuColors || colorStr.includes(\"__gpu\")) {\n        config = GRADIENT_CONFIGS.gpu;\n      } else if (colors === this._ramColors || colorStr.includes(\"__ram\")) {\n        config = GRADIENT_CONFIGS.ram;\n      } else if (colors === this._cpuColors || colorStr.includes(\"__cpu\")) {\n        config = GRADIENT_CONFIGS.cpu;"
        );
    }
    console.log("Colors already patched — skipping");
    return content;
  }

  // The original _getUsageColor method to replace.
  const originalMethod = `    _getUsageColor(value, colors) {
      return getUsageColor(value, colors, COLOR_LIST_SEPARATOR);
    }`;

  if (!content.includes(originalMethod)) {
    console.log("Could not find target _getUsageColor in extension.js — skipping");
    return content;
  }

  const classMarker = "export default class";
  if (!content.includes(classMarker)) {
    console.log("Could not find extension class marker in extension.js — skipping");
    return content;
  }

  const supportBlock = `// ── Gradient color support (patched by patch_resource_monitor_colors.js) ──
const ETHERNET_MAX_MBPS = ${ETHERNET_MAX_MBPS};
const RAM_MAX_GB = ${RAM_MAX_GB};
const DISK_USAGE_MAX_PERCENT = ${DISK_USAGE_MAX_PERCENT};
const GPU_MEMORY_MAX_GB = ${GPU_MEMORY_MAX_GB};

function rgbToStyle(r, g, b) {
  const rr = Math.max(0, Math.min(255, Math.round(r)));
  const gg = Math.max(0, Math.min(255, Math.round(g)));
  const bb = Math.max(0, Math.min(255, Math.round(b)));
  return \`color: rgb(\${rr}, \${gg}, \${bb});\`;
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
};`;

  // Build the replacement: gradient-based implementation + type marker injection.
  const replacement = `    // ── Gradient-based color override (patched by patch_resource_monitor_colors.js) ──
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
    }`;

  const withSupportBlock = content.replace(classMarker, `${supportBlock}\n\n${classMarker}`);
  const patched = withSupportBlock.replace(originalMethod, replacement);
  console.log("Patched extension.js with gradient-based color system");
  return patched;
}

// ── Entry point ───────────────────────────────────────────────────────────────

let content = fs.readFileSync(extPath, "utf8");
content = patchExtensionJS(content);
fs.writeFileSync(extPath, content);
