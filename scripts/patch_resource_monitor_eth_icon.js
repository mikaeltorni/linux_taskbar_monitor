// patch_resource_monitor_eth_icon.js — Drop the ethernet display icon from the
// panel while keeping the numeric Mbps value and its unit. The upstream code
// appends the icon for every simple metric group (cpu/ram/swap/disk/eth/wlan)
// through _appendSimpleChildren; there is no GSetting to hide a single icon, so
// this patch removes only the ethernet icon by making the eth call site skip
// the icon argument (the iconsPosition handling in _appendSimpleChildren then
// adds no icon at all).
//
// The edit is idempotent: re-running on an already-patched file is a no-op
// (guarded by a stable marker comment), and a fresh run on the upstream file
// applies the change. It fails fast when the expected upstream snippet is absent.
//
// Usage:
//   node scripts/patch_resource_monitor_eth_icon.js <path-to-mainGui.js>

const fs = require("fs");

const mainGuiPath = process.argv[2];
if (!mainGuiPath) {
  console.error("Usage: patch_resource_monitor_eth_icon.js <path-to-mainGui.js>");
  process.exit(1);
}

const content = fs.readFileSync(mainGuiPath, "utf8");

const ALREADY_MARKER = "Ethernet icon removed: value/unit kept, icon omitted";
if (content.includes(ALREADY_MARKER)) {
  console.log("Ethernet display icon already removed");
  process.exit(0);
}

// The eth group is wired to _appendSimpleChildren with the icon as its first
// argument. Drop that icon argument so no icon actor is created for ethernet;
// the value and unit labels remain. Matched literally for fail-fast behavior.
const oldSnippet = `  _replaceGroupChildren(indicator._ethGroup, (addChild) =>
    _appendSimpleChildren(
      indicator._ethIcon,
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

const patched = content.replace(oldSnippet, newSnippet);
fs.writeFileSync(mainGuiPath, patched);
console.log("Removed ethernet display icon (value/unit preserved)");
