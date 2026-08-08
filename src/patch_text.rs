//! Shared snippet-replacement helpers for idempotent extension source patches.
//!
//! Used by the Rust disk / text patchers so replacements stay fail-fast and
//! migration-aware (older patched forms can be upgraded to the current form).

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Fail-fast error when a required upstream snippet cannot be located.
#[derive(Debug, Error)]
#[error("Could not find target code in {0}")]
pub struct PatchTargetMissing(pub String);

/// Write `content` to `path` via a sibling `.tmp` file then rename.
///
/// Avoids leaving a truncated target when the process dies mid-write. Same-
/// filesystem rename is atomic on Linux for the final replace step.
pub fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp = {
        let mut os = path.as_os_str().to_owned();
        os.push(".tmp");
        PathBuf::from(os)
    };
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Migration from an older patched form to the current form.
///
/// # Fields
/// - `from`: Snippet that identifies the older patched form.
/// - `to`: Replacement text for the current form.
/// - `all`: When true, replace every occurrence of `from`; otherwise once.
/// - `required_marker`: Optional marker that must already be present before
///   the migration runs (guards against applying migrations to clean upstream).
#[derive(Debug, Clone)]
pub struct Migration {
    /// Older patched snippet to find.
    pub from: &'static str,
    /// Current patched form to write.
    pub to: &'static str,
    /// Replace all occurrences when true.
    pub all: bool,
    /// Optional prerequisite marker in the file.
    pub required_marker: Option<&'static str>,
}

/// Replace the first matching known snippet, or return the content unchanged
/// when `already_marker` is already present.
///
/// # Parameters
/// - `content`: File text to patch.
/// - `snippets`: Upstream forms to match (first hit wins).
/// - `replacement`: Current patched form.
/// - `already_marker`: Marker proving the patch is already applied.
/// - `target_name`: Human label used in logs and [`PatchTargetMissing`].
///
/// # Returns
/// `(content, status)` where status is `"already"` or `"patched"`.
pub fn replace_known_snippet(
    content: &str,
    snippets: &[&str],
    replacement: &str,
    already_marker: &str,
    target_name: &str,
) -> Result<(String, &'static str), PatchTargetMissing> {
    if content.contains(already_marker) {
        crate::logging::info(format!("{target_name} already patched"));
        return Ok((content.to_string(), "already"));
    }
    for snippet in snippets {
        if content.contains(snippet) {
            crate::logging::info(format!("Patched {target_name}"));
            return Ok((content.replacen(snippet, replacement, 1), "patched"));
        }
    }
    crate::logging::error(format!("Could not find target code in {target_name}"));
    Err(PatchTargetMissing(target_name.to_string()))
}

/// Apply in-place migrations first, then fall back to `replace_known_snippet`.
///
/// # Parameters
/// - `content`: File text to patch.
/// - `snippets`: Upstream forms to match after migrations.
/// - `replacement`: Current patched form.
/// - `already_marker`: Marker proving the current patch is present.
/// - `migrations`: Older patched forms to upgrade first.
/// - `target_name`: Human label used in logs and errors.
///
/// # Returns
/// `(content, status)` where status is `"migrated"`, `"already"`, or `"patched"`.
pub fn replace_known_snippet_with_migration(
    content: &str,
    snippets: &[&str],
    replacement: &str,
    already_marker: &str,
    migrations: &[Migration],
    target_name: &str,
) -> Result<(String, &'static str), PatchTargetMissing> {
    let mut migrated = content.to_string();
    let mut did_migrate = false;
    for migration in migrations {
        let required_ok = migration
            .required_marker
            .map(|m| migrated.contains(m))
            .unwrap_or(true);
        if required_ok && migrated.contains(migration.from) {
            if migration.all {
                migrated = migrated.replace(migration.from, migration.to);
            } else {
                migrated = migrated.replacen(migration.from, migration.to, 1);
            }
            did_migrate = true;
        }
    }
    if did_migrate {
        crate::logging::info(format!("Migrated {target_name}"));
        return Ok((migrated, "migrated"));
    }
    if content.contains(already_marker) {
        crate::logging::info(format!("{target_name} already patched"));
        return Ok((content.to_string(), "already"));
    }
    replace_known_snippet(content, snippets, replacement, already_marker, target_name)
}

/// Try regex patterns first, then literal snippets.
///
/// # Parameters
/// - `content`: File text to patch.
/// - `snippets`: Literal upstream forms (fallback after patterns).
/// - `patterns`: Compiled regexes tried before literals.
/// - `replacement`: Current patched form (literal; `${...}` is not expanded).
/// - `already_marker`: Marker proving the patch is already applied.
/// - `target_name`: Human label used in logs and errors.
///
/// # Returns
/// `(content, status)` where status is `"already"` or `"patched"`.
pub fn replace_known_snippet_or_patterns(
    content: &str,
    snippets: &[&str],
    patterns: &[regex::Regex],
    replacement: &str,
    already_marker: &str,
    target_name: &str,
) -> Result<(String, &'static str), PatchTargetMissing> {
    if content.contains(already_marker) {
        crate::logging::info(format!("{target_name} already patched"));
        return Ok((content.to_string(), "already"));
    }
    for pattern in patterns {
        if pattern.is_match(content) {
            crate::logging::info(format!("Patched {target_name}"));
            // NoExpand keeps `${...}` in injected JavaScript template literals
            // literal instead of being read as capture-group references.
            return Ok((
                pattern
                    .replace(content, regex::NoExpand(replacement))
                    .into_owned(),
                "patched",
            ));
        }
    }
    for snippet in snippets {
        if content.contains(snippet) {
            crate::logging::info(format!("Patched {target_name}"));
            return Ok((content.replacen(snippet, replacement, 1), "patched"));
        }
    }
    crate::logging::error(format!("Could not find target code in {target_name}"));
    Err(PatchTargetMissing(target_name.to_string()))
}

/// Like `replace_known_snippet`, but logs and skips when no target is found.
pub fn replace_known_snippet_optional(
    content: &str,
    snippets: &[&str],
    replacement: &str,
    already_marker: &str,
    target_name: &str,
) -> String {
    if content.contains(already_marker) {
        crate::logging::info(format!("{target_name} already patched"));
        return content.to_string();
    }
    for snippet in snippets {
        if content.contains(snippet) {
            crate::logging::info(format!("Patched {target_name}"));
            return content.replacen(snippet, replacement, 1);
        }
    }
    crate::logging::info(format!("{target_name} target not found; skipping"));
    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_marker_short_circuits() {
        let (out, status) =
            replace_known_snippet("aaMARKER", &["missing"], "x", "MARKER", "t").unwrap();
        assert_eq!(status, "already");
        assert_eq!(out, "aaMARKER");
    }

    #[test]
    fn replaces_first_snippet() {
        let (out, status) =
            replace_known_snippet("hello world", &["world"], "rust", "MARKER", "t").unwrap();
        assert_eq!(status, "patched");
        assert_eq!(out, "hello rust");
    }
}
