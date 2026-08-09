//! Show VRAM next to GPU usage % without brackets.
//!
//! Upstream Resource Monitor wraps the GPU memory (VRAM) value in `[ ]`
//! bracket labels; this patch replaces those brackets with a plain two-space
//! separator so VRAM sits directly beside the GPU usage percentage.
//!
//! Idempotent: a second run that already sees the space-separator marker
//! succeeds without rewriting the file. Missing both the upstream snippet and
//! the marker exits non-zero (unsupported extension version).

use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::logging;

const MARKER: &str = "Space separator between GPU usage and VRAM";

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

/// Failures when the upstream VRAM bracket anchors are missing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VramError {
    /// Neither the upstream bracket snippet nor the space-separator marker.
    #[error("Could not find target code in containers.js - unsupported extension version")]
    MissingTarget,
}

/// Remove the bracket labels around the GPU VRAM value.
///
/// # Parameters
/// - `content`: Current `containers.js` content.
///
/// Returns `(patched_content, changed)`. `changed` is false when the marker is
/// already present.
pub fn patch_containers(content: &str) -> Result<(String, bool), VramError> {
    if content.contains(MARKER) {
        return Ok((content.to_string(), false));
    }
    if !content.contains(OLD_CODE) {
        return Err(VramError::MissingTarget);
    }
    Ok((content.replacen(OLD_CODE, NEW_CODE, 1), true))
}

/// CLI entry point for the VRAM bracket patcher.
///
/// # Parameters
/// - `containers_path`: Path to the extension's `panel/containers.js`.
///
/// Returns `0` on success (including the already-patched case), `1` when the
/// file cannot be read/written or the target snippet is missing.
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

    let (patched, changed) = match patch_containers(&content) {
        Ok(result) => result,
        Err(err) => {
            logging::error(err.to_string());
            eprintln!("{err}");
            return 1;
        }
    };

    if !changed {
        logging::info("GPU VRAM brackets already removed");
        println!("GPU VRAM brackets already removed");
        return 0;
    }

    if let Err(err) = crate::patch_text::write_atomic(containers_path, &patched) {
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
        let (patched, changed) = patch_containers(&upstream()).expect("patched");
        assert!(changed);
        assert!(patched.contains(NEW_CODE));
        assert!(!patched.contains("_createBracketLabel"));
        assert!(patched.starts_with("prefix\n"));
        assert!(patched.ends_with("suffix\n"));
    }

    #[test]
    fn already_patched_content_is_a_no_op_success() {
        let (again, changed) = patch_containers(NEW_CODE).expect("already patched");
        assert!(!changed);
        assert_eq!(again, NEW_CODE);
    }

    #[test]
    fn unsupported_content_is_rejected() {
        assert_eq!(
            patch_containers("// unrelated"),
            Err(VramError::MissingTarget)
        );
    }

    #[test]
    fn run_is_idempotent_on_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("containers.js");
        fs::write(&path, upstream()).expect("write");
        assert_eq!(run(&path), 0);
        let first = fs::read_to_string(&path).expect("read");
        assert_eq!(run(&path), 0);
        assert_eq!(first, fs::read_to_string(&path).expect("read"));
    }

    #[test]
    fn run_exits_one_for_a_missing_file() {
        assert_eq!(run(Path::new("/nonexistent/rm-monitor/containers.js")), 1);
    }
}
