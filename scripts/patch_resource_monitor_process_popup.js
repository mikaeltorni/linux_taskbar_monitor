// patch_resource_monitor_process_popup.js — Replace the left-click "launch the
// configured action" behavior (which opens the GNOME task manager,
// gnome-system-monitor) with an in-panel popup menu that shows proper process
// data: total CPU% and RAM% aggregated per process name, plus the instance
// count, sorted by CPU descending.
//
// The upstream extension has no GSetting for this — left-click always spawns
// the leftclickstatus command — so this source patch:
//   1. Adds the PopupMenu import next to the existing PanelMenu import.
//   2. Inserts marker-guarded _toggleProcessMenu/_refreshProcessMenu methods
//      before _clickManager. Data comes from `ps -eo comm=,%cpu=,%mem=` read
//      asynchronously via Gio.Subprocess and aggregated by command name.
//   3. Rewires the left-click case of _clickManager (and the Enter/Space key
//      activation) from _launchPrimaryAction() to _toggleProcessMenu().
//   4. Updates the accessibility tooltip to describe the new behavior.
//
// The edits are idempotent (guarded by marker comments) and fail fast when the
// expected upstream snippets are absent.
//
// Usage:
//   node scripts/patch_resource_monitor_process_popup.js <path-to-extension.js>

const fs = require("fs");

const extensionPath = process.argv[2];
if (!extensionPath) {
  console.error(
    "Usage: patch_resource_monitor_process_popup.js <path-to-extension.js>"
  );
  process.exit(1);
}

let content = fs.readFileSync(extensionPath, "utf8");
let changed = false;

const MARKER =
  "Process popup: total CPU/RAM aggregated per process name";

// ── 1. PopupMenu import ──────────────────────────────────────────────────────
const PANEL_MENU_IMPORT =
  'import * as PanelMenu from "resource:///org/gnome/shell/ui/panelMenu.js";';
const POPUP_MENU_IMPORT =
  'import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";';
if (!content.includes(POPUP_MENU_IMPORT)) {
  if (!content.includes(PANEL_MENU_IMPORT)) {
    console.error(
      "ERROR: PanelMenu import not found; upstream layout changed — aborting."
    );
    process.exit(1);
  }
  content = content.replace(
    PANEL_MENU_IMPORT,
    `${PANEL_MENU_IMPORT}\n${POPUP_MENU_IMPORT}`
  );
  changed = true;
}

// ── 2. Popup-menu methods, inserted before _clickManager ─────────────────────
if (!content.includes(MARKER)) {
  const clickManagerAnchor = "    _clickManager(actor, event) {";
  if (!content.includes(clickManagerAnchor)) {
    console.error(
      "ERROR: _clickManager not found; upstream layout changed — aborting."
    );
    process.exit(1);
  }

  const methods = `    // ── ${MARKER} ──
    // Left-click no longer launches the task manager; it opens this menu,
    // whose rows show each process name with its summed CPU%, RAM%, and
    // instance count (ps aggregates per PID; we aggregate per name).
    _toggleProcessMenu() {
      if (!this.menu) {
        return;
      }

      if (this.menu.isOpen) {
        this.menu.close();
        return;
      }

      this._refreshProcessMenu();
      this.menu.open();
    }

    _refreshProcessMenu() {
      this.menu.removeAll();

      const loadingItem = new PopupMenu.PopupMenuItem(_("Loading\\u2026"), {
        reactive: false,
      });
      this.menu.addMenuItem(loadingItem);

      let proc;
      try {
        proc = new Gio.Subprocess({
          argv: ["ps", "-eo", "comm=,%cpu=,%mem="],
          flags:
            Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE,
        });
        proc.init(null);
      } catch (error) {
        this._logger.error(
          \`[Resource_Monitor] Error spawning ps for process popup: \${error}\`
        );
        loadingItem.label.text = _("Unable to read process list.");
        return;
      }

      proc.communicate_utf8_async(null, this._ioCancellable, (p, res) => {
        let stdout = "";
        try {
          [, stdout] = p.communicate_utf8_finish(res);
        } catch (error) {
          this._logger.error(
            \`[Resource_Monitor] Error reading ps output: \${error}\`
          );
          return;
        }

        if (this._destroyed || !this.menu || !this.menu.isOpen) {
          return;
        }

        this._populateProcessMenu(stdout ?? "");
      });
    }

    _populateProcessMenu(psOutput) {
      // Aggregate per command name: comm may contain spaces, but %cpu and
      // %mem are always the last two fields, so parse from the right.
      const totals = new Map();
      let totalCpu = 0;

      for (const line of psOutput.split("\\n")) {
        const fields = line.trim().split(/\\s+/);
        if (fields.length < 3) {
          continue;
        }

        const mem = parseFloat(fields.pop());
        const cpu = parseFloat(fields.pop());
        const name = fields.join(" ");
        if (name === "" || Number.isNaN(cpu) || Number.isNaN(mem)) {
          continue;
        }

        const entry = totals.get(name) ?? { cpu: 0, mem: 0, count: 0 };
        entry.cpu += cpu;
        entry.mem += mem;
        entry.count += 1;
        totals.set(name, entry);
        totalCpu += cpu;
      }

      const rows = [...totals.entries()]
        .sort((a, b) => b[1].cpu - a[1].cpu)
        .slice(0, 15);

      this.menu.removeAll();

      const header = new PopupMenu.PopupMenuItem(
        \`\${_("Top processes by CPU")} \\u2014 \${_("total")} \${totalCpu.toFixed(1)}%\`,
        { reactive: false }
      );
      header.label.set_style("font-weight: bold;");
      this.menu.addMenuItem(header);
      this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

      if (rows.length === 0) {
        this.menu.addMenuItem(
          new PopupMenu.PopupMenuItem(_("No process data available."), {
            reactive: false,
          })
        );
        return;
      }

      for (const [name, entry] of rows) {
        const displayName =
          name.length > 24 ? \`\${name.slice(0, 23)}\\u2026\` : name;
        const suffix = entry.count > 1 ? \` \\u00d7\${entry.count}\` : "";
        const text = \`\${displayName.padEnd(25)}\${entry.cpu
          .toFixed(1)
          .padStart(6)}% CPU \${entry.mem.toFixed(1).padStart(5)}% RAM\${suffix}\`;
        const item = new PopupMenu.PopupMenuItem(text, { reactive: false });
        item.label.set_style("font-family: monospace;");
        this.menu.addMenuItem(item);
      }
    }

`;
  content = content.replace(clickManagerAnchor, methods + clickManagerAnchor);
  changed = true;
}

// ── 3. Rewire left-click and keyboard activation ─────────────────────────────
const OLD_CLICK = `        case 1: // Left-click
          this._launchPrimaryAction();`;
const NEW_CLICK = `        case 1: // Left-click
          // Show per-process CPU/RAM data instead of launching the task manager.
          this._toggleProcessMenu();`;
if (content.includes(OLD_CLICK)) {
  content = content.replace(OLD_CLICK, NEW_CLICK);
  changed = true;
} else if (!content.includes(NEW_CLICK)) {
  console.error(
    "ERROR: left-click _launchPrimaryAction case not found — aborting."
  );
  process.exit(1);
}

const OLD_KEY = `        case Clutter.KEY_space:
          this._launchPrimaryAction();`;
const NEW_KEY = `        case Clutter.KEY_space:
          // Keyboard activation mirrors left-click: show the process popup.
          this._toggleProcessMenu();`;
if (content.includes(OLD_KEY)) {
  content = content.replace(OLD_KEY, NEW_KEY);
  changed = true;
} else if (!content.includes(NEW_KEY)) {
  console.error(
    "ERROR: keyboard _launchPrimaryAction case not found — aborting."
  );
  process.exit(1);
}

// ── 4. Tooltip describes the new behavior ────────────────────────────────────
const OLD_TOOLTIP = '_("Left-click launches the configured action.")';
const NEW_TOOLTIP = '_("Left-click shows per-process CPU and RAM usage.")';
if (content.includes(OLD_TOOLTIP)) {
  content = content.replace(OLD_TOOLTIP, NEW_TOOLTIP);
  changed = true;
}

if (!changed) {
  console.log("Process popup already applied; nothing to do.");
  process.exit(0);
}

fs.writeFileSync(extensionPath, content);
console.log(`Patched ${extensionPath}: left-click now shows the process popup.`);
