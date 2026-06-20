// patch_resource_monitor_vram.js — Show VRAM next to GPU usage % without brackets.
//
// The upstream Resource Monitor wraps the GPU memory (VRAM) value in "[ ]"
// bracket labels. This patch replaces those brackets with a plain two-space
// separator so VRAM sits directly beside the GPU usage percentage. The edit is
// fail-fast: if the expected upstream snippet is absent (already patched or an
// unsupported version) the script exits non-zero.
//
// Usage:
//   node scripts/patch_resource_monitor_vram.js <path-to-containers.js>

const fs = require('fs');

const containersPath = process.argv[2];
if (!containersPath) {
  console.error('Usage: patch_resource_monitor_vram.js <path-to-containers.js>');
  process.exit(1);
}

const content = fs.readFileSync(containersPath, 'utf8');

const oldCode = `        const separatorStart = _createBracketLabel("[", [
          "resource-monitor-secondary-bracket",
        ]);
        const separatorEnd = _createBracketLabel("]", [
          "resource-monitor-secondary-bracket",
        ]);
        this._separatorPairs.push({ start: separatorStart, end: separatorEnd });
        this.add_child(separatorStart);
        this.add_child(this._elementsMemoryValue[uuid]);
        this.add_child(this._elementsMemoryUnit[uuid]);
        this.add_child(separatorEnd);`;

const newCode = `        // Space separator between GPU usage and VRAM (no brackets)
        const spaceSep = new St.Label({ text: "  " });
        this.add_child(spaceSep);
        this.add_child(this._elementsMemoryValue[uuid]);
        this.add_child(this._elementsMemoryUnit[uuid]);`;

if (!content.includes(oldCode)) {
  console.error('Could not find target code in containers.js - patch may already be applied');
  process.exit(1);
}

const patched = content.replace(oldCode, newCode);
fs.writeFileSync(containersPath, patched);
console.log('Patched GPU VRAM display: removed brackets from memory section');
