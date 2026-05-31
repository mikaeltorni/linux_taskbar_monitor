// Patch Resource Monitor extension to show VRAM right next to GPU usage % (no brackets)
const fs = require('fs');
const path = require('path');

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
