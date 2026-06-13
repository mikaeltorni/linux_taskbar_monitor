// patch_text.js — Shared snippet-replacement helpers for the JavaScript patchers.
//
// These are pure text transforms used to apply idempotent, version-tolerant
// edits to upstream extension source files. Each helper logs progress and,
// where appropriate, exits the process when a required target cannot be found —
// preserving the fail-fast behavior the patchers depend on.

"use strict";

/**
 * Replace the first matching known snippet in a file's content with a replacement.
 * Exits the process if none of the snippets are found (and the patch is not already applied).
 * @param {string} content - Current file content.
 * @param {string[]} snippets - Candidate upstream snippets to look for, in priority order.
 * @param {string} replacement - Text to substitute for the matched snippet.
 * @param {string} alreadyMarker - Substring whose presence means the patch is already applied.
 * @param {string} targetName - Human-readable name used in log messages.
 * @returns {string} Patched content (or unchanged content when already patched).
 */
function replaceKnownSnippet(content, snippets, replacement, alreadyMarker, targetName) {
  if (content.includes(alreadyMarker)) {
    console.log(`${targetName} already patched`);
    return content;
  }

  for (const snippet of snippets) {
    if (content.includes(snippet)) {
      console.log(`Patched ${targetName}`);
      return content.replace(snippet, replacement);
    }
  }

  console.error(`Could not find target code in ${targetName}`);
  process.exit(1);
}

/**
 * Apply in-place migrations to older patched output before falling back to a
 * fresh snippet replacement. Lets the script upgrade content it patched in a
 * previous run without re-matching the pristine upstream snippet.
 * @param {string} content - Current file content.
 * @param {string[]} snippets - Pristine upstream snippets for first-time patching.
 * @param {string} replacement - Replacement text for first-time patching.
 * @param {string} alreadyMarker - Substring meaning the patch is already current.
 * @param {Array<{from: string, to: string, all?: boolean, requiredMarker?: string}>} migrations
 *   - Ordered migrations from older patched output to the current form.
 * @param {string} targetName - Human-readable name used in log messages.
 * @returns {string} Patched, migrated, or unchanged content.
 */
function replaceKnownSnippetWithMigration(
  content,
  snippets,
  replacement,
  alreadyMarker,
  migrations,
  targetName
) {
  let migratedContent = content;
  let didMigrate = false;
  for (const migration of migrations) {
    const requiredMarker = migration.requiredMarker || "";
    if (
      (!requiredMarker || migratedContent.includes(requiredMarker)) &&
      migratedContent.includes(migration.from)
    ) {
      migratedContent = migration.all
        ? migratedContent.split(migration.from).join(migration.to)
        : migratedContent.replace(migration.from, migration.to);
      didMigrate = true;
    }
  }

  if (didMigrate) {
    console.log(`Migrated ${targetName}`);
    return migratedContent;
  }

  if (content.includes(alreadyMarker)) {
    console.log(`${targetName} already patched`);
    return content;
  }

  return replaceKnownSnippet(content, snippets, replacement, alreadyMarker, targetName);
}

/**
 * Patch content by trying regex patterns first, then literal snippets.
 * Exits the process if neither matches (and the patch is not already applied).
 * @param {string} content - Current file content.
 * @param {string[]} snippets - Literal fallback snippets to look for.
 * @param {RegExp[]} patterns - Regex patterns tried before the literal snippets.
 * @param {string} replacement - Replacement text for the matched pattern/snippet.
 * @param {string} alreadyMarker - Substring meaning the patch is already applied.
 * @param {string} targetName - Human-readable name used in log messages.
 * @returns {string} Patched content (or unchanged content when already patched).
 */
function replaceKnownSnippetOrPatterns(
  content,
  snippets,
  patterns,
  replacement,
  alreadyMarker,
  targetName
) {
  if (content.includes(alreadyMarker)) {
    console.log(`${targetName} already patched`);
    return content;
  }

  for (const pattern of patterns) {
    if (pattern.test(content)) {
      console.log(`Patched ${targetName}`);
      return content.replace(pattern, replacement);
    }
  }

  for (const snippet of snippets) {
    if (content.includes(snippet)) {
      console.log(`Patched ${targetName}`);
      return content.replace(snippet, replacement);
    }
  }

  console.error(`Could not find target code in ${targetName}`);
  process.exit(1);
}

/**
 * Like replaceKnownSnippet, but tolerant: when no snippet matches it logs and
 * returns the content unchanged instead of exiting. Use for optional patches
 * whose target may legitimately be absent in some extension versions.
 * @param {string} content - Current file content.
 * @param {string[]} snippets - Candidate upstream snippets to look for.
 * @param {string} replacement - Replacement text for the matched snippet.
 * @param {string} alreadyMarker - Substring meaning the patch is already applied.
 * @param {string} targetName - Human-readable name used in log messages.
 * @returns {string} Patched content, or unchanged content when no target is found.
 */
function replaceKnownSnippetOptional(content, snippets, replacement, alreadyMarker, targetName) {
  if (content.includes(alreadyMarker)) {
    console.log(`${targetName} already patched`);
    return content;
  }

  for (const snippet of snippets) {
    if (content.includes(snippet)) {
      console.log(`Patched ${targetName}`);
      return content.replace(snippet, replacement);
    }
  }

  console.log(`${targetName} target not found; skipping`);
  return content;
}

module.exports = {
  replaceKnownSnippet,
  replaceKnownSnippetWithMigration,
  replaceKnownSnippetOrPatterns,
  replaceKnownSnippetOptional,
};
