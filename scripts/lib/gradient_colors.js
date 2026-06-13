// gradient_colors.js — Pure gradient color math for the Resource Monitor patch.
//
// This module is the canonical, unit-tested mirror of the gradient logic that
// patch_resource_monitor_colors.js injects into extension.js as a self-contained
// string block. GNOME Shell (GJS) cannot `require` Node modules, so the patcher
// must embed an inline copy; keeping this tested source alongside it documents
// and verifies the intended behavior.
//
// Gradient scheme: each metric maps a value within [minVal, maxVal] onto a
// green → yellow → red gradient, where green is healthy and red is critical.

"use strict";

/** Maximum Ethernet/WLAN throughput for color gradient, in displayed Mbps. */
const ETHERNET_MAX_MBPS = 2000;

/** Maximum RAM for color gradient, in GB. */
const RAM_MAX_GB = 64;

/** Maximum disk usage percentage for color gradient. */
const DISK_USAGE_MAX_PERCENT = 100;

/** Maximum GPU memory (VRAM) for color gradient, in GB. */
const GPU_MEMORY_MAX_GB = 24;

/**
 * Convert RGB values to a CSS color string, clamping to 0-255 and rounding.
 * @param {number} r - Red.
 * @param {number} g - Green.
 * @param {number} b - Blue.
 * @returns {string} CSS style string, e.g. "color: rgb(0, 255, 0);".
 */
function rgbToStyle(r, g, b) {
  const rr = Math.max(0, Math.min(255, Math.round(r)));
  const gg = Math.max(0, Math.min(255, Math.round(g)));
  const bb = Math.max(0, Math.min(255, Math.round(b)));
  return `color: rgb(${rr}, ${gg}, ${bb});`;
}

/**
 * Compute a linearly interpolated gradient color between two RGB endpoints.
 * @param {number} value - Current value (within minVal..maxVal).
 * @param {number} minVal - Minimum threshold value.
 * @param {number} maxVal - Maximum threshold value.
 * @param {number[]} startRGB - Start RGB [r, g, b].
 * @param {number[]} endRGB - End RGB [r, g, b].
 * @returns {string} CSS color style string, or "" when value is non-finite.
 */
function getGradientColor(value, minVal, maxVal, startRGB, endRGB) {
  if (!Number.isFinite(value)) return "";

  const ratio = (value - minVal) / (maxVal - minVal);
  const clampedRatio = Math.max(0, Math.min(1, ratio));
  const r = startRGB[0] + (endRGB[0] - startRGB[0]) * clampedRatio;
  const g = startRGB[1] + (endRGB[1] - startRGB[1]) * clampedRatio;
  const b = startRGB[2] + (endRGB[2] - startRGB[2]) * clampedRatio;

  return rgbToStyle(r, g, b);
}

/**
 * Compute a green → yellow → red gradient that passes through bright yellow at
 * the midpoint instead of a dark olive.
 * @param {number} value - Current value (within minVal..maxVal).
 * @param {number} minVal - Minimum threshold value.
 * @param {number} maxVal - Maximum threshold value.
 * @param {number[]} startRGB - Start RGB [r, g, b].
 * @param {number[]} endRGB - End RGB [r, g, b].
 * @returns {string} CSS color style string.
 */
function getGreenYellowRedGradientColor(value, minVal, maxVal, startRGB, endRGB) {
  const midpoint = minVal + (maxVal - minVal) / 2;
  const midpointRGB = [255, 255, 0];

  if (value <= midpoint) {
    return getGradientColor(value, minVal, midpoint, startRGB, midpointRGB);
  }

  return getGradientColor(value, midpoint, maxVal, midpointRGB, endRGB);
}

/**
 * Gradient configuration per indicator type. Each entry defines the value range
 * and the start (healthy/green) and end (critical/red) colors.
 */
const GRADIENT_CONFIGS = {
  cpu: { minVal: 0, maxVal: 100, startRGB: [0, 255, 0], endRGB: [255, 0, 0], inverted: false },
  ram: { minVal: 0, maxVal: RAM_MAX_GB, startRGB: [0, 255, 0], endRGB: [255, 0, 0], inverted: false },
  diskSpace: {
    minVal: 0,
    maxVal: DISK_USAGE_MAX_PERCENT,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
  eth: { minVal: 0, maxVal: ETHERNET_MAX_MBPS, startRGB: [0, 255, 0], endRGB: [255, 0, 0], inverted: false },
  wlan: { minVal: 0, maxVal: ETHERNET_MAX_MBPS, startRGB: [0, 255, 0], endRGB: [255, 0, 0], inverted: false },
  gpu: { minVal: 0, maxVal: 100, startRGB: [0, 255, 0], endRGB: [255, 0, 0], inverted: false },
  gpuMemory: {
    minVal: 0,
    maxVal: GPU_MEMORY_MAX_GB,
    startRGB: [0, 255, 0],
    endRGB: [255, 0, 0],
    inverted: false,
  },
};

/**
 * Gradient-based replacement for the extension's _getUsageColor method.
 *
 * Detects the indicator type by matching the colors argument against the
 * indicator's known color arrays (by identity) and falling back to appended
 * type markers (e.g. "__cpu"), then applies the matching gradient config. This
 * mirrors the inline method injected into extension.js, with the indicator
 * passed explicitly instead of via `this`.
 * @param {Object} indicator - The Resource Monitor indicator (its color arrays).
 * @param {*} value - The metric value (number or array; arrays take the max).
 * @param {string[]} colors - Color thresholds, possibly with type markers.
 * @returns {string} CSS style string, or "" when the value is non-finite.
 */
function gradientGetUsageColor(indicator, value, colors) {
  const numericValue = Array.isArray(value) ? Math.max(...value.filter(Number.isFinite)) : value;
  if (!Number.isFinite(numericValue)) return "";

  const colorStr = Array.isArray(colors) ? colors.join(" ") : String(colors || "");

  let config;
  if (colors === indicator._diskSpaceColors) {
    config = GRADIENT_CONFIGS.diskSpace;
  } else if (colors === indicator._netEthColors || colorStr.includes("__eth")) {
    config = GRADIENT_CONFIGS.eth;
  } else if (colors === indicator._netWlanColors || colorStr.includes("__wlan")) {
    config = GRADIENT_CONFIGS.wlan;
  } else if (colors === indicator._gpuMemoryColors || colorStr.includes("__gpuMem")) {
    config = GRADIENT_CONFIGS.gpuMemory;
  } else if (colorStr.includes("__diskSpace")) {
    config = GRADIENT_CONFIGS.diskSpace;
  } else if (colors === indicator._gpuColors || colorStr.includes("__gpu")) {
    config = GRADIENT_CONFIGS.gpu;
  } else if (colors === indicator._ramColors || colorStr.includes("__ram")) {
    config = GRADIENT_CONFIGS.ram;
  } else if (colors === indicator._cpuColors || colorStr.includes("__cpu")) {
    config = GRADIENT_CONFIGS.cpu;
  } else {
    config = GRADIENT_CONFIGS.cpu;
  }

  let effectiveValue = numericValue;
  const minVal = config.minVal;
  const maxVal = config.maxVal;

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

/**
 * Build a green → yellow → red CSS color style for a disk usage percentage.
 * Unlike getGreenYellowRedGradientColor this is a fixed 0-100 percentage scale
 * with a `color: rgb(r, g, 0);` form. Mirrors the helper injected into
 * refreshers.js by patch_resource_monitor_disk.js.
 * @param {number} value - Usage percentage (0-100). Non-finite values yield "".
 * @returns {string} A `color: rgb(r, g, 0);` style string, or "" when invalid.
 */
function getDiskUsagePercentStyle(value) {
  if (!Number.isFinite(value)) {
    return "";
  }

  const ratio = Math.max(0, Math.min(1, value / 100));
  const red = ratio <= 0.5 ? Math.round(510 * ratio) : 255;
  const green = ratio <= 0.5 ? 255 : Math.round(510 * (1 - ratio));

  return `color: rgb(${red}, ${green}, 0);`;
}

module.exports = {
  ETHERNET_MAX_MBPS,
  RAM_MAX_GB,
  DISK_USAGE_MAX_PERCENT,
  GPU_MEMORY_MAX_GB,
  rgbToStyle,
  getGradientColor,
  getGreenYellowRedGradientColor,
  GRADIENT_CONFIGS,
  gradientGetUsageColor,
  getDiskUsagePercentStyle,
};
