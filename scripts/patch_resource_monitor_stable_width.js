// patch_resource_monitor_stable_width.js — Reserve (or release) fixed value-label
// widths so the panel does not shift as metric values change digit count (e.g.
// CPU 9% -> 10% -> 100%, disk activity 5% -> 100%, or GPU VRAM 8 -> 24 GB).
//
// The extension's upstream *width GSettings already reserve the primary value
// labels (cpuwidth, ramwidth, diskspacewidth, netethwidth, gpuwidth), but two
// labels have no such GSetting and are left adaptive:
//   1. The disk-space secondary "activity %" value.
//   2. The GPU VRAM value — upstream shares the single gpuwidth with the GPU
//      usage percentage, even though VRAM is at most 2 digits while usage is 3.
// This patcher reserves both through the same mechanism the extension uses for
// the primary values — setting element.width (which St honors and which is
// right-aligned by the value label) — rather than relying on a CSS min-width
// rule, which St does not reliably apply to St.Label actors.
//
// The edit is idempotent: re-running on an already-patched file is a no-op
// (guarded by stable marker comments and literal-snippet checks), and a fresh
// run on the upstream file applies the reservations.
//
// Modes
// -----
//   stable  (default) Apply the fixed-width reservations so the panel stays
//                    put as values change digit count ("stable spacing").
//   compact           Remove the reservations so the value labels are adaptive.
//                    The indicator then takes less horizontal space, but the
//                    panel shifts slightly as a reading grows/shrinks (e.g.
//                    CPU 9% -> 100%). This is the "no space between the
//                    components" look the user can opt into.
//
// Usage:
//   node scripts/patch_resource_monitor_stable_width.js [--mode stable|compact] <path-to-containers.js>

const fs = require("fs");

const MODES = ["stable", "compact"];
let mode = "stable";
let containersPath = process.argv[2];

// Parse an optional leading --mode flag (default: stable).
if (process.argv[2] === "--mode") {
  mode = process.argv[3];
  containersPath = process.argv[4];
} else if (typeof process.argv[2] === "string" && process.argv[2].startsWith("--mode=")) {
  mode = process.argv[2].slice("--mode=".length);
  containersPath = process.argv[3];
}

if (!MODES.includes(mode)) {
  console.error("Invalid --mode \"" + mode + "\". Use one of: " + MODES.join(", "));
  process.exit(1);
}

if (!containersPath) {
  console.error("Usage: patch_resource_monitor_stable_width.js [--mode stable|compact] <path-to-containers.js>");
  process.exit(1);
}

let content = fs.readFileSync(containersPath, "utf8");
let changed = false;

// ── 1. Disk-space secondary activity % width ────────────────────────────────
// Already patched (stable): the secondary activity value width is reserved via a
// marker. compact mode removes that reservation so the value is adaptive again.
const DISK_MARKER =
  "Space separator between disk-space activity percent and its unit (stable width)";
if (mode === "compact") {
  if (content.includes(DISK_MARKER)) {
    // Remove the disk-space secondary-width reservation and restore the
    // upstream (adaptive) add_element ordering.
    const compactAdd = `      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      // Space separator between disk-space activity percent and its unit (stable width)
      this._elementsSecondaryValue[filesystem].width = this._diskActivityWidth;
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);`;
    const upstreamAdd = `      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);`;
    if (!content.includes(compactAdd)) {
      console.error("Could not find disk-space stable-width edit to remove in containers.js");
      process.exit(1);
    }
    content = content.replace(compactAdd, upstreamAdd);

    // Drop the _diskActivityWidth constant from DiskContainerSpace._init.
    const patchedInit = `  class DiskContainerSpace extends DiskContainer {
    _init() {
      super._init();

      // Stable reserved width (px, pre-scale) for the disk-space secondary
      // activity percentage so it does not shift the panel as digits change.
      // Covers the 3-digit worst case "100" at the panel font with slack;
      // secondary values render at 0.92em so this comfortably fits it.
      this._diskActivityWidth = 24;

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }`;
    const upstreamInit = `  class DiskContainerSpace extends DiskContainer {
    _init() {
      super._init();

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }`;
    if (!content.includes(patchedInit)) {
      console.error("Could not find DiskContainerSpace._init stable-width constant to remove");
      process.exit(1);
    }
    content = content.replace(patchedInit, upstreamInit);
    changed = true;
    console.log("Released disk-space activity percent width (compact mode)");
  } else {
    console.log("Disk-space activity percent width already compact");
  }
} else if (!content.includes(DISK_MARKER)) {
  // Upstream DiskContainerSpace.add_element builds the secondary value followed
  // by the secondary unit, separated by a two-space gap. We inject a stable-width
  // reservation between the two actors. The snippet is matched literally so the
  // patch fails fast when the upstream source changes.
  const oldSnippet = `      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);`;

  if (!content.includes(oldSnippet)) {
    console.error("Could not find disk-space target code in containers.js - patch may be already applied or unsupported version");
    process.exit(1);
  }

  const newSnippet = `      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      // Space separator between disk-space activity percent and its unit (stable width)
      this._elementsSecondaryValue[filesystem].width = this._diskActivityWidth;
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);`;

  content = content.replace(oldSnippet, newSnippet);

  // Declare the disk activity width constant in DiskContainerSpace._init so the
  // reservation is available when add_element runs.
  const oldInit = `  class DiskContainerSpace extends DiskContainer {
    _init() {
      super._init();

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }`;

  const newInit = `  class DiskContainerSpace extends DiskContainer {
    _init() {
      super._init();

      // Stable reserved width (px, pre-scale) for the disk-space secondary
      // activity percentage so it does not shift the panel as digits change.
      // Covers the 3-digit worst case "100" at the panel font with slack;
      // secondary values render at 0.92em so this comfortably fits it.
      this._diskActivityWidth = 24;

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }`;

  if (!content.includes(oldInit)) {
    console.error("Could not find DiskContainerSpace._init in containers.js");
    process.exit(1);
  }

  content = content.replace(oldInit, newInit);
  changed = true;
  console.log("Reserved disk-space activity percent width (stable-width patch)");
}

// ── 2. GPU VRAM width split from GPU usage ───────────────────────────────────
// Upstream GpuContainer.set_element_width assigns the same width to the usage
// value and the VRAM value. VRAM is at most 2 digits (0-99 GB) while usage is
// 3, so give VRAM its own tighter reserved width. compact mode reverts VRAM to
// share the GPU usage width again (adaptive alongside the usage value).
// Already patched (stable) guard:
const GPU_MARKER = "VRAM value (0-99 GB, 2 digits) gets its own tighter reserved";
if (mode === "compact") {
  if (content.includes(GPU_MARKER)) {
    // Restore the VRAM value to the shared width in set_element_width.
    const patchedSet = `      } else {
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
      }`;
    const upstreamSet = `      } else {
        this._elementsUuid.forEach((element) => {
          if (this._elementsValue[element] !== undefined) {
            this._elementsValue[element].width = width;
          }

          if (this._elementsMemoryValue[element] !== undefined) {
            this._elementsMemoryValue[element].width = width;
          }
        });
      }`;
    if (!content.includes(patchedSet)) {
      console.error("Could not find GpuContainer.set_element_width VRAM edit to remove");
      process.exit(1);
    }
    content = content.replace(patchedSet, upstreamSet);

    // Drop the _gpuMemoryWidth constant from GpuContainer._init.
    const patchedInit = `  class GpuContainer extends St.BoxLayout {
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
    }`;
    const upstreamInit = `  class GpuContainer extends St.BoxLayout {
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
    }`;
    if (!content.includes(patchedInit)) {
      console.error("Could not find GpuContainer._init VRAM width constant to remove");
      process.exit(1);
    }
    content = content.replace(patchedInit, upstreamInit);
    changed = true;
    console.log("Released GPU VRAM width back to shared GPU usage width (compact mode)");
  } else {
    console.log("GPU VRAM width already compact");
  }
} else if (!content.includes(GPU_MARKER)) {
  // Insert the VRAM width constant into GpuContainer._init. The class header
  // makes this snippet unique (the same field list appears in cleanup_elements
  // but without the `class GpuContainer extends St.BoxLayout {` prefix).
  const oldInit = `  class GpuContainer extends St.BoxLayout {
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
    }`;

  const newInit = `  class GpuContainer extends St.BoxLayout {
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
    }`;

  if (!content.includes(oldInit)) {
    console.error("Could not find GpuContainer._init in containers.js");
    process.exit(1);
  }

  content = content.replace(oldInit, newInit);

  // Route the VRAM value through the new constant instead of the shared width.
  const oldSet = `      } else {
        this._elementsUuid.forEach((element) => {
          if (this._elementsValue[element] !== undefined) {
            this._elementsValue[element].width = width;
          }

          if (this._elementsMemoryValue[element] !== undefined) {
            this._elementsMemoryValue[element].width = width;
          }
        });
      }`;

  const newSet = `      } else {
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
      }`;

  if (!content.includes(oldSet)) {
    console.error("Could not find GpuContainer.set_element_width else-branch in containers.js");
    process.exit(1);
  }

  content = content.replace(oldSet, newSet);
  changed = true;
  console.log("Split GPU usage / VRAM reserved widths (stable-width patch)");
}

if (!changed) {
  console.log(mode === "compact" ? "Stable widths already compact" : "Stable widths already reserved");
}

fs.writeFileSync(containersPath, content);
