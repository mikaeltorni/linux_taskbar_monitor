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

/// Compute the sibling `.tmp` path used by [`write_atomic`] and
/// [`write_atomic_batch`] for `path`.
fn tmp_sibling(path: &Path) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(".tmp");
    PathBuf::from(os)
}

/// Write `content` to `path` via a sibling `.tmp` file then rename.
///
/// Avoids leaving a truncated target when the process dies mid-write. Same-
/// filesystem rename is atomic on Linux for the final replace step.
pub fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp = tmp_sibling(path);
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Write a batch of `(path, content)` pairs atomically as a group.
///
/// Every file's `.tmp` sibling is written first; only once **all** writes
/// succeed are the files renamed into place (also in order). This keeps a
/// multi-file patch (e.g. `containers.js` + `refreshers.js` + `extension.js`)
/// from leaving some targets patched and others untouched when a later file
/// in the batch fails to write.
///
/// On a tmp-write failure, every `.tmp` sibling created so far in this call is
/// removed (best-effort) and the error is returned without renaming any file
/// — the targets stay exactly as they were before the call.
pub fn write_atomic_batch(files: &[(PathBuf, String)]) -> std::io::Result<()> {
    let mut created_tmps: Vec<PathBuf> = Vec::with_capacity(files.len());
    for (path, content) in files {
        let tmp = tmp_sibling(path);
        if let Err(err) = fs::write(&tmp, content) {
            for created in &created_tmps {
                let _ = fs::remove_file(created);
            }
            return Err(err);
        }
        created_tmps.push(tmp);
    }
    for ((path, _), tmp) in files.iter().zip(created_tmps.iter()) {
        fs::rename(tmp, path)?;
    }
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
/// A marker alone does not prove the patched *body* is current: a manual edit
/// or an older codegen pass can leave the marker in place while drifting the
/// surrounding text away from `replacement`. So when the marker is present
/// but `replacement` is not fully there, this still checks for a known old
/// `snippets` form to upgrade from. Only when the marker is present, the
/// current `replacement` is absent, *and* none of the known old `snippets`
/// forms are present either does this fall back to leaving the content
/// untouched — there is nothing safe to replace from, and failing here would
/// wrongly treat a merely-differently-formatted current body as broken.
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
    let marker_present = content.contains(already_marker);
    if marker_present
        && (content.contains(replacement) || snippets.iter().all(|s| !content.contains(s)))
    {
        crate::logging::info(format!("{target_name} already patched"));
        return Ok((content.to_string(), "already"));
    }
    for snippet in snippets {
        if content.contains(snippet) {
            crate::logging::info(format!(
                "{}{target_name}",
                if marker_present {
                    "Upgrading stale body of "
                } else {
                    "Patched "
                }
            ));
            return Ok((content.replacen(snippet, replacement, 1), "patched"));
        }
    }
    crate::logging::error(format!("Could not find target code in {target_name}"));
    Err(PatchTargetMissing(target_name.to_string()))
}

/// Outcome of [`replace_stale_block`].
#[derive(Debug, PartialEq, Eq)]
pub enum StaleBlockOutcome {
    /// The block already matches `replacement`; nothing to do.
    Unchanged,
    /// The block differed from `replacement` and was replaced.
    Replaced(String),
    /// `start_marker` or `end_anchor` could not be located, so the block's
    /// bounds are unknown. Callers with a marker-implies-bounds invariant
    /// should treat this as a hard failure rather than silently skip.
    AnchorsMissing,
}

/// Replace the text between `start_marker` (inclusive) and `end_anchor`
/// (exclusive) with `replacement` when it differs, preserving whatever
/// trailing whitespace originally separated the block from `end_anchor`.
///
/// Used to upgrade a marker-guarded block whose *body* has drifted from the
/// current constant even though the guard marker is still present (e.g. a
/// stale error-handling branch inside an otherwise-recognized injected
/// function). `end_anchor` must be stable content that this patch does not
/// itself rewrite, so it keeps pointing at the right boundary across patch
/// versions.
///
/// # Parameters
/// - `content`: File text to inspect.
/// - `start_marker`: Text marking the start of the block (e.g. a comment
///   line that is also the first line of `replacement`).
/// - `end_anchor`: Stable text immediately following the block.
/// - `replacement`: Current form of the block (without the trailing
///   whitespace that separates it from `end_anchor`).
pub fn replace_stale_block(
    content: &str,
    start_marker: &str,
    end_anchor: &str,
    replacement: &str,
) -> StaleBlockOutcome {
    let Some(start) = content.find(start_marker) else {
        return StaleBlockOutcome::AnchorsMissing;
    };
    let Some(rel_end) = content[start..].find(end_anchor) else {
        return StaleBlockOutcome::AnchorsMissing;
    };
    let end = start + rel_end;
    let current = &content[start..end];
    let trimmed_len = current.trim_end().len();
    if &current[..trimmed_len] == replacement {
        return StaleBlockOutcome::Unchanged;
    }
    let trailing = &current[trimmed_len..];
    StaleBlockOutcome::Replaced(format!(
        "{}{replacement}{trailing}{}",
        &content[..start],
        &content[end..]
    ))
}

/// Like [`replace_stale_block`], but `end_anchor` is the block's own stable
/// tail rather than unrelated content that follows it, so it is included
/// (not excluded) in the compared/replaced span. Useful for a multi-`{}`
/// block (several sibling methods/statements) that has no single balanced
/// delimiter pair spanning the whole thing, but does end in a known-stable
/// snippet (e.g. a small forwarding method that must always stay present).
///
/// # Parameters
/// - `content`: File text to inspect.
/// - `start_marker`: Text marking the start of the block.
/// - `end_anchor`: Stable text forming the tail of the block itself.
/// - `replacement`: Current form of the block, ending in `end_anchor`.
pub fn replace_stale_block_including_anchor(
    content: &str,
    start_marker: &str,
    end_anchor: &str,
    replacement: &str,
) -> StaleBlockOutcome {
    let Some(start) = content.find(start_marker) else {
        return StaleBlockOutcome::AnchorsMissing;
    };
    let Some(rel_end) = content[start..].find(end_anchor) else {
        return StaleBlockOutcome::AnchorsMissing;
    };
    let end = start + rel_end + end_anchor.len();
    let current = &content[start..end];
    if current == replacement {
        return StaleBlockOutcome::Unchanged;
    }
    StaleBlockOutcome::Replaced(format!(
        "{}{replacement}{}",
        &content[..start],
        &content[end..]
    ))
}

/// Like [`replace_stale_block`], but the block is a single self-terminating
/// balanced-delimiter span (e.g. a whole `function ... { ... }` or a
/// balanced `foo(...)` call) rather than text bounded by unrelated following
/// content. This needs no assumption about what follows the block in the
/// file, which keeps it correct even in minimal fixtures.
///
/// The block spans from the start of `start_marker` through the `close`
/// delimiter that balances the first `open` delimiter found at or after
/// `start_marker` (nesting-aware), plus one trailing `extra_trailing`
/// character when present immediately after (e.g. the `;` closing a
/// statement-level call).
///
/// # Parameters
/// - `content`: File text to inspect.
/// - `start_marker`: Text marking the start of the block.
/// - `open`/`close`: Delimiter pair bounding the block (e.g. `'{'`/`'}'`).
/// - `extra_trailing`: Optional single character consumed right after the
///   closing delimiter when present (e.g. `';'`).
/// - `replacement`: Current form of the block, spanning exactly
///   `start_marker` through the closing delimiter (and `extra_trailing`).
pub fn replace_stale_balanced_block(
    content: &str,
    start_marker: &str,
    open: char,
    close: char,
    extra_trailing: Option<char>,
    replacement: &str,
) -> StaleBlockOutcome {
    let Some(start) = content.find(start_marker) else {
        return StaleBlockOutcome::AnchorsMissing;
    };
    let Some(rel_open) = content[start..].find(open) else {
        return StaleBlockOutcome::AnchorsMissing;
    };
    let open_idx = start + rel_open;
    let Some(close_idx) = find_matching_close(content, open_idx, open, close) else {
        return StaleBlockOutcome::AnchorsMissing;
    };
    let mut end = close_idx + close.len_utf8();
    if let Some(extra) = extra_trailing {
        if content[end..].starts_with(extra) {
            end += extra.len_utf8();
        }
    }
    let current = &content[start..end];
    if current == replacement {
        return StaleBlockOutcome::Unchanged;
    }
    StaleBlockOutcome::Replaced(format!(
        "{}{replacement}{}",
        &content[..start],
        &content[end..]
    ))
}

/// Find the index of the `close` delimiter that balances the `open`
/// delimiter at `open_idx`, tracking nesting depth so inner pairs (e.g. an
/// arrow-function's own `(...)`  inside a call's argument list) do not end
/// the scan early. `content` as of `open_idx` must start with `open`.
fn find_matching_close(content: &str, open_idx: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0i32;
    for (idx, ch) in content[open_idx..].char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(open_idx + idx);
            }
        }
    }
    None
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
        // Style-only migrations can succeed without converging to the current
        // body. Keep going until the already_marker is present.
        if migrated.contains(already_marker) {
            crate::logging::info(format!("Migrated {target_name}"));
            return Ok((migrated, "migrated"));
        }
        crate::logging::info(format!(
            "Partial migration of {target_name}; applying current replacement"
        ));
        return replace_known_snippet(
            &migrated,
            snippets,
            replacement,
            already_marker,
            target_name,
        );
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

    #[test]
    fn marker_with_stale_body_is_upgraded_from_a_known_old_snippet() {
        // The marker is present (so a naive check would short-circuit), but
        // the surrounding body still matches an older known form rather
        // than the current replacement — it must be upgraded, not skipped.
        let (out, status) = replace_known_snippet(
            "prefix OLD_BODY MARKER suffix",
            &["OLD_BODY"],
            "NEW_BODY",
            "MARKER",
            "t",
        )
        .unwrap();
        assert_eq!(status, "patched");
        assert_eq!(out, "prefix NEW_BODY MARKER suffix");
    }

    #[test]
    fn marker_with_unrecognized_body_and_no_known_old_snippet_is_left_alone() {
        // Marker present, current replacement absent, and none of the known
        // old snippets match either — nothing safe to replace from, so this
        // must stay "already" rather than corrupt or reject the content.
        let (out, status) = replace_known_snippet(
            "prefix UNRECOGNIZED_BODY MARKER suffix",
            &["OLD_BODY"],
            "NEW_BODY",
            "MARKER",
            "t",
        )
        .unwrap();
        assert_eq!(status, "already");
        assert_eq!(out, "prefix UNRECOGNIZED_BODY MARKER suffix");
    }

    #[test]
    fn replace_stale_block_replaces_a_stale_body_and_preserves_trailing_whitespace() {
        let content = "prefix\nSTART\nold body\nEND\nsuffix";
        match replace_stale_block(content, "START\n", "END", "START\nnew body") {
            StaleBlockOutcome::Replaced(out) => {
                assert_eq!(out, "prefix\nSTART\nnew body\nEND\nsuffix");
            }
            other => panic!("expected Replaced, got {other:?}"),
        }
    }

    #[test]
    fn replace_stale_block_reports_unchanged_when_body_already_matches() {
        let content = "prefix\nSTART\ncurrent body\nEND\nsuffix";
        assert_eq!(
            replace_stale_block(content, "START\n", "END", "START\ncurrent body"),
            StaleBlockOutcome::Unchanged
        );
    }

    #[test]
    fn replace_stale_block_reports_anchors_missing() {
        assert_eq!(
            replace_stale_block("no markers here", "START\n", "END", "x"),
            StaleBlockOutcome::AnchorsMissing
        );
        assert_eq!(
            replace_stale_block("START\nbody, no end anchor", "START\n", "END", "x"),
            StaleBlockOutcome::AnchorsMissing
        );
    }

    #[test]
    fn replace_stale_block_including_anchor_replaces_up_to_and_including_the_tail() {
        let content = "prefix\nSTART\nold body\nTAIL\nafter";
        match replace_stale_block_including_anchor(
            content,
            "START\n",
            "TAIL",
            "START\nnew body\nTAIL",
        ) {
            StaleBlockOutcome::Replaced(out) => {
                assert_eq!(out, "prefix\nSTART\nnew body\nTAIL\nafter");
            }
            other => panic!("expected Replaced, got {other:?}"),
        }
    }

    #[test]
    fn replace_stale_block_including_anchor_reports_unchanged_and_anchors_missing() {
        let content = "START\ncurrent body\nTAIL\nafter";
        assert_eq!(
            replace_stale_block_including_anchor(
                content,
                "START\n",
                "TAIL",
                "START\ncurrent body\nTAIL"
            ),
            StaleBlockOutcome::Unchanged
        );
        assert_eq!(
            replace_stale_block_including_anchor("no markers", "START\n", "TAIL", "x"),
            StaleBlockOutcome::AnchorsMissing
        );
    }

    #[test]
    fn replace_stale_balanced_block_replaces_a_stale_function_body() {
        let content = "before\nfunction f() {\n  old();\n}\nafter";
        match replace_stale_balanced_block(
            content,
            "function f() {",
            '{',
            '}',
            None,
            "function f() {\n  new();\n}",
        ) {
            StaleBlockOutcome::Replaced(out) => {
                assert_eq!(out, "before\nfunction f() {\n  new();\n}\nafter");
            }
            other => panic!("expected Replaced, got {other:?}"),
        }
    }

    #[test]
    fn replace_stale_balanced_block_handles_nested_delimiters() {
        // The outer call's own opening paren must balance against its own
        // closing paren, not the inner arrow function's parens.
        let content = "wire(target, (x) =>\n  inner(x)\n);\nafter";
        match replace_stale_balanced_block(
            content,
            "wire(target,",
            '(',
            ')',
            Some(';'),
            "wire(target, (x) =>\n  inner2(x)\n);",
        ) {
            StaleBlockOutcome::Replaced(out) => {
                assert_eq!(out, "wire(target, (x) =>\n  inner2(x)\n);\nafter");
            }
            other => panic!("expected Replaced, got {other:?}"),
        }
    }

    #[test]
    fn replace_stale_balanced_block_reports_unchanged_and_anchors_missing() {
        let content = "function f() {\n  body();\n}";
        assert_eq!(
            replace_stale_balanced_block(
                content,
                "function f() {",
                '{',
                '}',
                None,
                "function f() {\n  body();\n}",
            ),
            StaleBlockOutcome::Unchanged
        );
        assert_eq!(
            replace_stale_balanced_block("no function here", "function f() {", '{', '}', None, "x"),
            StaleBlockOutcome::AnchorsMissing
        );
    }

    #[test]
    fn write_atomic_replaces_via_tmp_sibling() {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("target.txt");
        write_atomic(&path, "one").expect("write");
        assert_eq!(fs::read_to_string(&path).unwrap(), "one");
        assert!(!path.with_extension("txt.tmp").exists());
        write_atomic(&path, "two").expect("rewrite");
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
        assert!(
            !tmp_sibling(&path).exists(),
            "tmp sibling must be renamed away"
        );
    }

    #[test]
    fn write_atomic_batch_writes_all_tmps_then_renames_all() {
        let dir = tempfile::tempdir().expect("temp");
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        write_atomic_batch(&[(a.clone(), "A".to_string()), (b.clone(), "B".to_string())])
            .expect("batch write");
        assert_eq!(fs::read_to_string(&a).unwrap(), "A");
        assert_eq!(fs::read_to_string(&b).unwrap(), "B");
        assert!(!tmp_sibling(&a).exists());
        assert!(!tmp_sibling(&b).exists());
    }

    #[test]
    fn write_atomic_batch_cleans_up_tmps_and_renames_nothing_on_write_failure() {
        let dir = tempfile::tempdir().expect("temp");
        let a = dir.path().join("a.txt");
        // Parent directory does not exist, so writing this tmp sibling fails.
        let unwritable = dir.path().join("missing-dir").join("b.txt");
        let result = write_atomic_batch(&[
            (a.clone(), "A".to_string()),
            (unwritable.clone(), "B".to_string()),
        ]);

        assert!(result.is_err());
        assert!(!a.exists(), "earlier target must not be renamed into place");
        assert!(!unwritable.exists());
        assert!(
            !tmp_sibling(&a).exists(),
            "tmp created before the failure must be cleaned up"
        );
        assert!(!tmp_sibling(&unwritable).exists());
    }
}
