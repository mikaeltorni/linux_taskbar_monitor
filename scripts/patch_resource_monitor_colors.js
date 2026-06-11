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

const extPath = process.argv[2];
if (!extPath) {
  console.error("Usage: patch_resource_monitor_colors.js <path-to-extension.js>");
  process.exit(1);
}

// ── Configuration variables (adjust these to change gradient ranges) ─────────

/** Maximum Ethernet/WLAN throughput for color gradient, in displayed Mbps (megabits per second). */
const ETHERNET_MAX_MBPS = 2000;

/** Maximum RAM for color gradient, in GB. */
const RAM_MAX_GB = 64;

/** Maximum disk usage percentage for color gradient. */
const DISK_USAGE_MAX_PERCENT = 100;

/** Maximum GPU memory (VRAM) for color gradient, in GB. */
const GPU_MEMORY_MAX_GB = 24;

// ── Color helpers ────────────────────────────────────────────────────────────

/**
 * Convert RGB values to a CSS color string.
 * @param {number} r - Red (0-255)
 * @param {number} g - Green (0-255)
 * @param {number} b - Blue (0-255)
 * @returns {string} CSS style string with color property.
 */
function rgbToStyle(r, g, b) {
  const rr = Math.max(0, Math.min(255, Math.round(r)));
  const gg = Math.max(0, Math.min(255, Math.round(g)));
  const bb = Math.max(0, Math.min(255, Math.round(b)));
  return `color: rgb(${rr}, ${gg}, ${bb});`;
}

/**
 * Compute a gradient color between two RGB endpoints.
 * Uses linear interpolation for smooth transitions across the value range.
 * @param {number} value - Current value (within minVal..maxVal).
 * @param {number} minVal - Minimum threshold value.
 * @param {number} maxVal - Maximum threshold value.
 * @param {number[]} startRGB - Start RGB [r, g, b].
 * @param {number[]} endRGB - End RGB [r, g, b].
 * @returns {string} CSS color style string.
 */
function getGradientColor(value, minVal, maxVal, startRGB, endRGB) {
  if (!Number.isFinite(value)) return "";

  const ratio = (value - minVal) / (maxVal - minVal);
  const clampedRatio = Math.max(0, Math.min(1, ratio));

  // Linear interpolation across the gradient.
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

// ── Gradient configuration per indicator type ────────────────────────────────

/**
 * Configuration for each indicator's color gradient.
 * Each entry defines:
 *   - minVal / maxVal: The value range (e.g., 0-100 for CPU, 0-64GB for RAM).
 *   - startRGB: Color at minimum (green = healthy).
 *   - endRGB: Color at maximum (red = critical).
 *   - inverted: If true, green at max and red at min (for disk space free).
 */
const GRADIENT_CONFIGS = {
  cpu: {
    label: "CPU",
    minVal: 0,
    maxVal: 100,
    startRGB: [0, 255, 0],   // Green at 0% usage
    endRGB: [255, 0, 0],     // Red at 100% usage
    inverted: false,
  },
  ram: {
    label: "RAM",
    minVal: 0,
    maxVal: RAM_MAX_GB,
    startRGB: [0, 255, 0],   // Green at 0GB used
    endRGB: [255, 0, 0],     // Red at maxGB used
    inverted: false,
  },
  diskSpace: {
    label: "Disk Space",
    minVal: 0,
    maxVal: DISK_USAGE_MAX_PERCENT,
    startRGB: [0, 255, 0],   // Green at 0% used
    endRGB: [255, 0, 0],     // Red at 100% used
    inverted: false,
  },
  eth: {
    label: "Ethernet",
    minVal: 0,
    maxVal: ETHERNET_MAX_MBPS,
    startRGB: [0, 255, 0],   // Green at 0 Mb/s
    endRGB: [255, 0, 0],     // Red at max Mb/s
    inverted: false,
  },
  wlan: {
    label: "Wi-Fi",
    minVal: 0,
    maxVal: ETHERNET_MAX_MBPS,
    startRGB: [0, 255, 0],   // Green at 0 Mb/s
    endRGB: [255, 0, 0],     // Red at max Mb/s
    inverted: false,
  },
  gpu: {
    label: "GPU",
    minVal: 0,
    maxVal: 100,             // GPU usage percentage (configurable via nvidia-smi)
    startRGB: [0, 255, 0],   // Green at 0%
    endRGB: [255, 0, 0],     // Red at max%
    inverted: false,
  },
  gpuMemory: {
    label: "GPU Memory",
    minVal: 0,
    maxVal: GPU_MEMORY_MAX_GB,
    startRGB: [0, 255, 0],   // Green at 0GB used
    endRGB: [255, 0, 0],     // Red at maxVRAM used
    inverted: false,
  },
};

// ── Gradient-based _getUsageColor override ────────────────────────────────────

/**
 * Override for the extension's _getUsageColor method.
 * Uses configurable RGB gradients instead of threshold-based coloring.
 *
 * Detection strategy: Each indicator's colors array is augmented with a type marker
 * (e.g., "__cpu", "__ram") during initialization. This function parses those markers
 * to reliably identify which metric is being colored, then applies the appropriate
 * gradient from GRADIENT_CONFIGS.
 *
 * @param {Object} indicator - The Resource Monitor indicator object.
 * @param {*} value - The metric value (number or array).
 * @param {string[]} colors - Original color thresholds with type markers appended.
 * @returns {string} CSS style string with the computed color.
 */
function _gradientGetUsageColor(indicator, value, colors) {
  // Handle array values (e.g., ethernet [download, upload]) — take max.
  const numericValue = Array.isArray(value) ? Math.max(...value.filter(Number.isFinite)) : value;
  if (!Number.isFinite(numericValue)) return "";

  // Detect indicator type from the colors parameter's type markers.
  // Each color entry is like "0% green" — we look for __type markers appended by init.
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
    // Fallback: CPU gradient for usage percentages.
    config = GRADIENT_CONFIGS.cpu;
  }

  let effectiveValue = numericValue;
  let minVal = config.minVal;
  let maxVal = config.maxVal;

  // Apply inverted logic (e.g., disk space: green when full, red when empty).
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

// ── Main patching function ───────────────────────────────────────────────────

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
