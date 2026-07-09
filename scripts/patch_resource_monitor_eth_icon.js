// patch_resource_monitor_eth_icon.js — Drop the ethernet display icon from the
// panel while keeping the numeric Mbps value and its unit. The upstream code
// appends the icon for every simple metric group (cpu/ram/swap/disk/eth/wlan)
// through _appendSimpleChildren; there is no GSetting to hide a single icon.
//
// This patch does two things:
//   1. Guards _appendSimpleChildren so a null icon adds no actor (St's addChild
//      rejects null). Without this, passing a null icon would throw on load.
//   2. Wires the ethernet group to _appendSimpleChildren with a null icon, so
//      no icon actor is created for ethernet while its value/unit labels remain.
//
// The edits are idempotent (guarded by marker comments) and fail fast when the
// expected upstream snippets are absent.
//
// Usage:
//   node scripts/patch_resource_monitor_eth_icon.js <path-to-mainGui.js>

const fs = require("fs");

const mainGuiPath = process.argv[2];
if (!mainGuiPath) {
  console.error("Usage: patch_resource_monitor_eth_icon.js <path-to-mainGui.js>");
  process.exit(1);
}

let content = fs.readFileSync(mainGuiPath, "utf8");
let changed = false;

// ── 1. Guard _appendSimpleChildren against a null icon ───────────────────────
const ICON_GUARD_MARKER =
  "A null icon (e.g. ethernet) means no icon actor is added";
if (!content.includes(ICON_GUARD_MARKER)) {
  const oldFn = `function _appendSimpleChildren(icon, value, unit, addChild, iconsPosition) {
  if (iconsPosition === "left") {
    addChild(icon);
  }

  addChild(value);
  addChild(unit);

  if (iconsPosition !== "left") {
    addChild(icon);
  }
}`;

  const newFn = `function _appendSimpleChildren(icon, value, unit, addChild, iconsPosition) {
  // A null icon (e.g. ethernet) means no icon actor is added.
  if (icon) {
    if (iconsPosition === "left") {
      addChild(icon);
    }
  }

  addChild(value);
  addChild(unit);

  if (icon) {
    if (iconsPosition !== "left") {
      addChild(icon);
    }
  }
}`;

  if (!content.includes(oldFn)) {
    console.error("Could not find _appendSimpleChildren in mainGui.js - patch may be already applied or unsupported version");
    process.exit(1);
  }

  content = content.replace(oldFn, newFn);
  changed = true;
  console.log("Guarded _appendSimpleChildren against null icon (eth-icon patch)");
}

// ── 2. Drop the eth icon argument at its wiring site ────────────────────────
const ETH_MARKER = "Ethernet icon removed: value/unit kept, icon omitted";
if (!content.includes(ETH_MARKER)) {
  const oldSnippet = `  _replaceGroupChildren(indicator._ethGroup, (addChild) =>
    _appendSimpleChildren(
      indicator._ethIcon,
      indicator._ethValue,
      indicator._ethUnit,
      addChild,
      iconsPosition
    )
  );`;

  const newSnippet = `  _replaceGroupChildren(indicator._ethGroup, (addChild) =>
    // Ethernet icon removed: value/unit kept, icon omitted
    _appendSimpleChildren(
      null,
      indicator._ethValue,
      indicator._ethUnit,
      addChild,
      iconsPosition
    )
  );`;

  if (!content.includes(oldSnippet)) {
    console.error("Could not find ethernet group wiring in mainGui.js - patch may be already applied or unsupported version");
    process.exit(1);
  }

  content = content.replace(oldSnippet, newSnippet);
  changed = true;
  console.log("Removed ethernet display icon (value/unit preserved)");
}

if (!changed) {
  console.log("Ethernet display icon already removed");
}

fs.writeFileSync(mainGuiPath, content);
