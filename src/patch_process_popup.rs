//! Replace the left-click "launch the configured action" behavior with an
//! in-panel process popup.
//!
//! Upstream always
//! spawns the `leftclickstatus` command on left-click and offers no GSetting to
//! change that, so this source patch:
//!
//! 1. Adds the `PopupMenu` import next to the existing `PanelMenu` import.
//! 2. Inserts marker-guarded `_toggleProcessMenu`/`_refreshProcessMenu` methods
//!    before `_clickManager`, reading `ps -eo comm=,%cpu=,%mem=` asynchronously
//!    and aggregating per command name.
//! 3. Rewires the left-click case of `_clickManager` and the Enter/Space key
//!    activation to open the popup.
//! 4. Updates the accessibility tooltip to describe the new behavior.
//!
//! The edits are idempotent (guarded by marker comments) and fail fast when the
//! expected upstream snippets are absent.

use std::fs;
use std::path::Path;

use crate::logging;

/// Marker comment proving the popup methods are already injected.
const MARKER: &str = "Process popup: total CPU/RAM aggregated per process name";

const PANEL_MENU_IMPORT: &str =
    r#"import * as PanelMenu from "resource:///org/gnome/shell/ui/panelMenu.js";"#;

const POPUP_MENU_IMPORT: &str =
    r#"import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";"#;

const CLICK_MANAGER_ANCHOR: &str = "    _clickManager(actor, event) {";

/// Popup methods injected before `_clickManager`. Keep the marker comment in
/// sync with [`MARKER`]; a unit test enforces that.
const METHODS: &str = r#"    // ── Process popup: total CPU/RAM aggregated per process name ──
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

      const loadingItem = new PopupMenu.PopupMenuItem(_("Loading\u2026"), {
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
          `[Resource_Monitor] Error spawning ps for process popup: ${error}`
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
            `[Resource_Monitor] Error reading ps output: ${error}`
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

      for (const line of psOutput.split("\n")) {
        const fields = line.trim().split(/\s+/);
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
        `${_("Top processes by CPU")} \u2014 ${_("total")} ${totalCpu.toFixed(1)}%`,
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
          name.length > 24 ? `${name.slice(0, 23)}\u2026` : name;
        const suffix = entry.count > 1 ? ` \u00d7${entry.count}` : "";
        const text = `${displayName.padEnd(25)}${entry.cpu
          .toFixed(1)
          .padStart(6)}% CPU ${entry.mem.toFixed(1).padStart(5)}% RAM${suffix}`;
        const item = new PopupMenu.PopupMenuItem(text, { reactive: false });
        item.label.set_style("font-family: monospace;");
        this.menu.addMenuItem(item);
      }
    }

"#;

const OLD_CLICK: &str = r#"        case 1: // Left-click
          this._launchPrimaryAction();"#;

const NEW_CLICK: &str = r#"        case 1: // Left-click
          // Show per-process CPU/RAM data instead of launching the task manager.
          this._toggleProcessMenu();"#;

const OLD_KEY: &str = r#"        case Clutter.KEY_space:
          this._launchPrimaryAction();"#;

const NEW_KEY: &str = r#"        case Clutter.KEY_space:
          // Keyboard activation mirrors left-click: show the process popup.
          this._toggleProcessMenu();"#;

const OLD_TOOLTIP: &str = r#"_("Left-click launches the configured action.")"#;

const NEW_TOOLTIP: &str = r#"_("Left-click shows per-process CPU and RAM usage.")"#;

/// Failure while applying the process-popup patch.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)] // Missing* names mirror the absent snippet
pub enum ProcessPopupError {
    /// The `PanelMenu` import anchor is missing.
    #[error("ERROR: PanelMenu import not found; upstream layout changed — aborting.")]
    MissingPanelMenuImport,
    /// The `_clickManager` anchor is missing.
    #[error("ERROR: _clickManager not found; upstream layout changed — aborting.")]
    MissingClickManager,
    /// Neither the upstream nor the patched left-click case is present.
    #[error("ERROR: left-click _launchPrimaryAction case not found — aborting.")]
    MissingLeftClickCase,
    /// Neither the upstream nor the patched keyboard case is present.
    #[error("ERROR: keyboard _launchPrimaryAction case not found — aborting.")]
    MissingKeyboardCase,
}

/// Apply every process-popup edit to `extension.js` content.
///
/// # Parameters
/// - `content`: Current `extension.js` content.
///
/// Returns the patched content and whether anything changed.
pub fn patch_extension_js(content: &str) -> Result<(String, bool), ProcessPopupError> {
    let mut content = content.to_string();
    let mut changed = false;

    // ── 1. PopupMenu import ──────────────────────────────────────────────
    if !content.contains(POPUP_MENU_IMPORT) {
        if !content.contains(PANEL_MENU_IMPORT) {
            return Err(ProcessPopupError::MissingPanelMenuImport);
        }
        content = content.replacen(
            PANEL_MENU_IMPORT,
            &format!("{PANEL_MENU_IMPORT}\n{POPUP_MENU_IMPORT}"),
            1,
        );
        changed = true;
    }

    // ── 2. Popup-menu methods, inserted before _clickManager ─────────────
    if !content.contains(MARKER) {
        if !content.contains(CLICK_MANAGER_ANCHOR) {
            return Err(ProcessPopupError::MissingClickManager);
        }
        content = content.replacen(
            CLICK_MANAGER_ANCHOR,
            &format!("{METHODS}{CLICK_MANAGER_ANCHOR}"),
            1,
        );
        changed = true;
    }

    // ── 3. Rewire left-click and keyboard activation ─────────────────────
    if content.contains(OLD_CLICK) {
        content = content.replacen(OLD_CLICK, NEW_CLICK, 1);
        changed = true;
    } else if !content.contains(NEW_CLICK) {
        return Err(ProcessPopupError::MissingLeftClickCase);
    }

    if content.contains(OLD_KEY) {
        content = content.replacen(OLD_KEY, NEW_KEY, 1);
        changed = true;
    } else if !content.contains(NEW_KEY) {
        return Err(ProcessPopupError::MissingKeyboardCase);
    }

    // ── 4. Tooltip describes the new behavior ────────────────────────────
    if content.contains(OLD_TOOLTIP) {
        content = content.replacen(OLD_TOOLTIP, NEW_TOOLTIP, 1);
        changed = true;
    }

    Ok((content, changed))
}

/// CLI entry point for the process-popup patcher.
///
/// # Parameters
/// - `extension_path`: Path to the extension's `extension.js`.
///
/// Returns `0` on success (including the already-applied case) and `1` when an
/// upstream anchor is missing or the file cannot be read/written.
pub fn run(extension_path: &Path) -> i32 {
    logging::info(format!(
        "patch-process-popup path={}",
        extension_path.display()
    ));

    let content = match fs::read_to_string(extension_path) {
        Ok(content) => content,
        Err(err) => {
            logging::error(format!(
                "Could not read {}: {err}",
                extension_path.display()
            ));
            eprintln!("Could not read {}: {err}", extension_path.display());
            return 1;
        }
    };

    let (patched, changed) = match patch_extension_js(&content) {
        Ok(result) => result,
        Err(err) => {
            logging::error(err.to_string());
            eprintln!("{err}");
            return 1;
        }
    };

    if !changed {
        logging::info("Process popup already applied; nothing to do.");
        println!("Process popup already applied; nothing to do.");
        return 0;
    }

    if let Err(err) = fs::write(extension_path, patched) {
        logging::error(format!(
            "Could not write {}: {err}",
            extension_path.display()
        ));
        eprintln!("Could not write {}: {err}", extension_path.display());
        return 1;
    }

    logging::info(format!(
        "Patched {}: left-click now shows the process popup.",
        extension_path.display()
    ));
    println!(
        "Patched {}: left-click now shows the process popup.",
        extension_path.display()
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream() -> String {
        format!(
            "{PANEL_MENU_IMPORT}\n\n\
             {CLICK_MANAGER_ANCHOR}\n\
             {OLD_CLICK}\n\
             {OLD_KEY}\n\
             const tooltip = {OLD_TOOLTIP};\n"
        )
    }

    #[test]
    fn injected_methods_carry_the_idempotence_marker() {
        assert!(METHODS.contains(MARKER));
        assert!(METHODS.ends_with("    }\n\n"));
    }

    #[test]
    fn injected_methods_keep_javascript_escapes_literal() {
        // These must reach extension.js as backslash escapes for GJS to parse.
        assert!(METHODS.contains(r#"psOutput.split("\n")"#));
        assert!(METHODS.contains(r"line.trim().split(/\s+/)"));
        assert!(METHODS.contains(r#"_("Loading\u2026")"#));
        assert!(METHODS.contains(r"\u2014"));
        assert!(METHODS.contains(r"\u00d7${entry.count}"));
        assert!(METHODS.contains("`[Resource_Monitor] Error reading ps output: ${error}`"));
    }

    #[test]
    fn applies_every_edit_to_upstream_source() {
        let (patched, changed) = patch_extension_js(&upstream()).expect("patch");
        assert!(changed);
        assert!(patched.contains(POPUP_MENU_IMPORT));
        assert!(patched.contains(MARKER));
        assert!(patched.contains(NEW_CLICK));
        assert!(patched.contains(NEW_KEY));
        assert!(patched.contains(NEW_TOOLTIP));
        assert!(!patched.contains("this._launchPrimaryAction();"));
        // The methods must land immediately before _clickManager.
        let methods_at = patched.find(MARKER).expect("marker");
        let anchor_at = patched.find(CLICK_MANAGER_ANCHOR).expect("anchor");
        assert!(methods_at < anchor_at);
    }

    #[test]
    fn re_running_the_patch_changes_nothing() {
        let (once, _) = patch_extension_js(&upstream()).expect("first");
        let (twice, changed) = patch_extension_js(&once).expect("second");
        assert!(!changed);
        assert_eq!(once, twice);
    }

    #[test]
    fn missing_panel_menu_import_fails_fast() {
        assert!(matches!(
            patch_extension_js("// unrelated"),
            Err(ProcessPopupError::MissingPanelMenuImport)
        ));
    }

    #[test]
    fn missing_click_manager_fails_fast() {
        assert!(matches!(
            patch_extension_js(PANEL_MENU_IMPORT),
            Err(ProcessPopupError::MissingClickManager)
        ));
    }

    #[test]
    fn missing_click_and_key_cases_fail_fast() {
        let no_click = format!("{PANEL_MENU_IMPORT}\n{CLICK_MANAGER_ANCHOR}\n{OLD_KEY}\n");
        assert!(matches!(
            patch_extension_js(&no_click),
            Err(ProcessPopupError::MissingLeftClickCase)
        ));

        let no_key = format!("{PANEL_MENU_IMPORT}\n{CLICK_MANAGER_ANCHOR}\n{OLD_CLICK}\n");
        assert!(matches!(
            patch_extension_js(&no_key),
            Err(ProcessPopupError::MissingKeyboardCase)
        ));
    }

    #[test]
    fn run_is_idempotent_on_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("extension.js");
        fs::write(&path, upstream()).expect("write");
        assert_eq!(run(&path), 0);
        let first = fs::read_to_string(&path).expect("read");
        assert_eq!(run(&path), 0);
        assert_eq!(first, fs::read_to_string(&path).expect("read"));
    }
}
