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
//   - Disk Space: maxSpace(405GB) green → 0 free red (inverted — full=green, empty=red)
//   - Ethernet:   0 MB/s green → ETHERNET_MAX_MBPS MB/s red for both upload & download
//   - Wi-Fi:      same as Ethernet
//   - GPU Usage:  0% green → 100% red (percentage-based, like CPU)
//   - GPU Memory: 0GB green → maxVRAM(nvidia-smi) red (configurable via GSettings)
//
// Configuration variables (change these to adjust gradients):
//   ETHERNET_MAX_MBPS    — Ethernet/WLAN color gradient max in MB/s (default: 2000)
//   RAM_MAX_GB           — RAM color gradient max in GB (default: 64)
//   DISK_SPACE_MAX_GB    — Disk space color gradient max in GB (default: 405)
//   GPU_MEMORY_MAX_GB    — GPU memory color gradient max in GB (default: 24)
//
// Usage:
//   node scripts/patch_resource_monitor_colors.js <path-to-extension.js>

const fs = require("fs");
const path = require("path");

const extPath = process.argv[2];
if (!extPath) {
  console.error("Usage: patch_resource_monitor_colors.js <path-to-extension.js>");
  process.exit(1);
}

// ── Configuration variables (adjust these to change gradient ranges) ───────────

/** Maximum Ethernet/WLAN throughput for color gradient, in MB/s. */
const ETHERNET_MAX_MBPS = 2000;

/** Maximum RAM for color gradient, in GB. */
const RAM_MAX_GB = 64;

/** Maximum disk space for color gradient, in GB. */
const DISK_SPACE_MAX_GB = 405;

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
    maxVal: RAM_MAX_GB,      // Configurable via ETHERNET_MAX_MBPS variable
    startRGB: [0, 255, 0],   // Green at 0GB used
    endRGB: [255, 0, 0],     // Red at maxGB used
    inverted: false,
  },
  diskSpace: {
    label: "Disk Space",
    minVal: 0,
    maxVal: DISK_SPACE_MAX_GB,  // Configurable via DISK_SPACE_MAX_GB variable
    startRGB: [0, 255, 0],   // Green when full (max free space)
    endRGB: [255, 0, 0],     // Red when empty (0 free space)
    inverted: true,          // Inverted: green at max, red at min
  },
  eth: {
    label: "Ethernet",
    minVal: 0,
    maxVal: ETHERNET_MAX_MBPS,   // MB/s — configurable via ETHERNET_MAX_MBPS variable
    startRGB: [0, 255, 0],   // Green at 0 MB/s (idle)
    endRGB: [255, 0, 0],     // Red at max MB/s
    inverted: false,
  },
  wlan: {
    label: "Wi-Fi",
    minVal: 0,
    maxVal: ETHERNET_MAX_MBPS,   // MB/s — configurable via ETHERNET_MAX_MBPS variable
    startRGB: [0, 255, 0],   // Green at 0 MB/s (idle)
    endRGB: [255, 0, 0],     // Red at max MB/s
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
    maxVal: GPU_MEMORY_MAX_GB,   // GB — configurable via GPU_MEMORY_MAX_GB variable
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
  if (!Number.isFinite(value)) return "";

  // Handle array values (e.g., ethernet [download, upload]) — take max.
  const numericValue = Array.isArray(value) ? Math.max(...value.filter(Number.isFinite)) : value;
  if (!Number.isFinite(numericValue)) return "";

  // Detect indicator type from the colors parameter's type markers.
  // Each color entry is like "0% green" — we look for __type markers appended by init.
  const colorStr = Array.isArray(colors) ? colors.join(" ") : String(colors || "");

  let config;
  if (colorStr.includes("__eth")) {
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

  return getGradientColor(effectiveValue, minVal, maxVal, config.startRGB, config.endRGB);
}

// ── Main patching function ───────────────────────────────────────────────────

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
      if (!Number.isFinite(value)) return "";

      const numericValue = Array.isArray(value) ? Math.max(...value.filter(Number.isFinite)) : value;
      if (!Number.isFinite(numericValue)) return "";

      // Detect indicator type from the colors parameter's type markers.
      const colorStr = Array.isArray(colors) ? colors.join(" ") : String(colors || "");

      let config;
      if (colorStr.includes("__eth")) {
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

      return getGradientColor(effectiveValue, minVal, maxVal, config.startRGB, config.endRGB);
    }

    _getUsageColor(value, colors) {
      return this._gradientGetUsageColor(value, colors);
    }`;

  const patched = content.replace(originalMethod, replacement);
  console.log("Patched extension.js with gradient-based color system");
  return patched;
}

// ── Entry point ───────────────────────────────────────────────────────────────

let content = fs.readFileSync(extPath, "utf8");
content = patchExtensionJS(content);
fs.writeFileSync(extPath, content);
