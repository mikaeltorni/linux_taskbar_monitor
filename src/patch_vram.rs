//! Show VRAM next to GPU usage % without brackets.
//!
//! Port of `scripts/patch_resource_monitor_vram.js`. Upstream Resource Monitor
//! wraps the GPU memory (VRAM) value in `[ ]` bracket labels; this patch
//! replaces those brackets with a plain two-space separator so VRAM sits
//! directly beside the GPU usage percentage.
//!
//! The edit is fail-fast: when the expected upstream snippet is absent (already
//! patched, or an unsupported version) the command exits non-zero.

use std::fs;
use std::path::Path;

use crate::logging;

const OLD_CODE: &str = r#"        const separatorStart = _createBracketLabel("[", [
          "resource-monitor-secondary-bracket",
        ]);
        const separatorEnd = _createBracketLabel("]", [
          "resource-monitor-secondary-bracket",
        ]);
        this._separatorPairs.push({ start: separatorStart, end: separatorEnd });
        this.add_child(separatorStart);
        this.add_child(this._elementsMemoryValue[uuid]);
        this.add_child(this._elementsMemoryUnit[uuid]);
        this.add_child(separatorEnd);"#;

const NEW_CODE: &str = r#"        // Space separator between GPU usage and VRAM (no brackets)
        const spaceSep = new St.Label({ text: "  " });
        this.add_child(spaceSep);
        this.add_child(this._elementsMemoryValue[uuid]);
        this.add_child(this._elementsMemoryUnit[uuid]);"#;

/// Remove the bracket labels around the GPU VRAM value.
///
/// # Parameters
/// - `content`: Current `containers.js` content.
///
/// Returns the patched content, or `None` when the upstream snippet is absent.
pub fn patch_containers(content: &str) -> Option<String> {
    if !content.contains(OLD_CODE) {
        return None;
    }
    Some(content.replacen(OLD_CODE, NEW_CODE, 1))
}

/// CLI entry point for the VRAM bracket patcher.
///
/// # Parameters
/// - `containers_path`: Path to the extension's `panel/containers.js`.
///
/// Returns `0` on success, `1` when the file cannot be read/written or the
/// target snippet is missing (which includes the already-patched case).
pub fn run(containers_path: &Path) -> i32 {
    logging::info(format!("patch-vram path={}", containers_path.display()));

    let content = match fs::read_to_string(containers_path) {
        Ok(content) => content,
        Err(err) => {
            logging::error(format!(
                "Could not read {}: {err}",
                containers_path.display()
            ));
            eprintln!("Could not read {}: {err}", containers_path.display());
            return 1;
        }
    };

    let Some(patched) = patch_containers(&content) else {
        logging::error("Could not find target code in containers.js - patch may already be applied");
        eprintln!("Could not find target code in containers.js - patch may already be applied");
        return 1;
    };

    if let Err(err) = fs::write(containers_path, patched) {
        logging::error(format!(
            "Could not write {}: {err}",
            containers_path.display()
        ));
        eprintln!("Could not write {}: {err}", containers_path.display());
        return 1;
    }

    logging::info("Patched GPU VRAM display: removed brackets from memory section");
    println!("Patched GPU VRAM display: removed brackets from memory section");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream() -> String {
        format!("prefix\n{OLD_CODE}\nsuffix\n")
    }

    #[test]
    fn replaces_the_bracket_labels() {
        let patched = patch_containers(&upstream()).expect("patched");
        assert!(patched.contains(NEW_CODE));
        assert!(!patched.contains("_createBracketLabel"));
        assert!(patched.starts_with("prefix\n"));
        assert!(patched.ends_with("suffix\n"));
    }

    #[test]
    fn already_patched_content_is_rejected() {
        assert!(patch_containers(NEW_CODE).is_none());
    }

    #[test]
    fn run_exits_one_when_already_applied() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("containers.js");
        fs::write(&path, upstream()).expect("write");
        assert_eq!(run(&path), 0);
        // A second run finds no bracket labels and must fail fast.
        assert_eq!(run(&path), 1);
    }

    #[test]
    fn run_exits_one_for_a_missing_file() {
        assert_eq!(run(Path::new("/nonexistent/rm-monitor/containers.js")), 1);
    }
}
