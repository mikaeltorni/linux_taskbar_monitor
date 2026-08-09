//! GSettings string-array helpers for the installer (`gsettings-strv` CLI).

use std::collections::HashSet;
use std::env;

/// Parse `gsettings get` string-array output into normalized values.
///
/// # Parameters
/// - `raw`: Optional `gsettings get` output (`@as [...]` or `[...]`). `None`
///   and empty strings yield an empty list.
///
/// # Errors
/// Returns `Err` when `raw` is present but is not a parseable list literal, so
/// callers never treat garbage as an empty array (which would wipe GSettings).
pub fn try_parse_strv(raw: Option<&str>) -> Result<Vec<String>, &'static str> {
    let mut value = raw.unwrap_or("").trim().to_string();
    if let Some(rest) = value.strip_prefix("@as ") {
        value = rest.trim().to_string();
    }
    if value.is_empty() {
        return Ok(Vec::new());
    }
    pythonish_list(&value).ok_or("CURRENT is not a parseable GSettings string-array")
}

/// Parse a Python/gsettings list literal like `['a', "b"]` (UTF-8 safe).
fn pythonish_list(value: &str) -> Option<Vec<String>> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return None;
    }
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }

    let mut items = Vec::new();
    let mut chars = inner.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() || ch == ',' {
            chars.next();
            continue;
        }
        let quote = chars.next()?;
        if quote != '\'' && quote != '"' {
            return None;
        }
        let mut out = String::new();
        let mut closed = false;
        while let Some(c) = chars.next() {
            if c == '\\' {
                let escaped = chars.next()?;
                out.push(escaped);
                continue;
            }
            if c == quote {
                closed = true;
                break;
            }
            out.push(c);
        }
        if !closed {
            return None;
        }
        items.push(out);
    }
    Some(items)
}

/// De-duplicate while preserving first appearance.
///
/// # Parameters
/// - `values`: Input list that may contain duplicates.
pub fn unique_values(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for v in values {
        if seen.insert(v.clone()) {
            out.push(v);
        }
    }
    out
}

/// Append `value` when non-empty and not already present.
///
/// # Parameters
/// - `raw`: Current `gsettings get` string-array text (or `None`).
/// - `value`: Entry to append.
///
/// # Errors
/// Propagates [`try_parse_strv`] failures for unparseable `raw`.
pub fn append_strv(raw: Option<&str>, value: &str) -> Result<Vec<String>, &'static str> {
    let mut current = unique_values(try_parse_strv(raw)?);
    if !value.is_empty() && !current.iter().any(|v| v == value) {
        current.push(value.to_string());
    }
    Ok(current)
}

/// Remove every occurrence of `value`.
///
/// # Parameters
/// - `raw`: Current `gsettings get` string-array text (must be set for CLI
///   remove; empty/`None` is allowed for library callers that already validated).
/// - `value`: Entry to drop.
///
/// # Errors
/// Propagates [`try_parse_strv`] failures for unparseable `raw`.
pub fn remove_strv(raw: Option<&str>, value: &str) -> Result<Vec<String>, &'static str> {
    Ok(unique_values(
        try_parse_strv(raw)?
            .into_iter()
            .filter(|item| item != value)
            .collect(),
    ))
}

/// Serialize values for `gsettings set` (Python `repr` style single quotes).
///
/// Escapes `\` and `'` only — matching the installer's historical contract for
/// extension UUIDs and similar simple tokens (not a full Python `repr`).
///
/// # Parameters
/// - `values`: Normalized string-array entries.
pub fn format_strv(values: &[String]) -> String {
    let parts: Vec<String> = values
        .iter()
        .map(|item| format!("'{}'", item.replace('\\', "\\\\").replace('\'', "\\'")))
        .collect();
    format!("[{}]", parts.join(", "))
}

/// CLI entry: `append|remove <value>` reading `CURRENT` from the environment.
///
/// # Parameters
/// - `action`: Must be `append` or `remove`.
/// - `value`: Entry to add or drop.
///
/// # Returns
/// `Ok(())` after printing the new list on stdout. `Err(2)` for an invalid
/// action (usage error). `Err(1)` when `CURRENT` is set but unparseable, or
/// when `remove` is invoked without `CURRENT` (avoids inventing `[]` and wiping
/// a live GSettings list).
pub fn run(action: &str, value: &str) -> Result<(), i32> {
    crate::logging::info(format!("gsettings-strv {action} value={value}"));
    if action != "append" && action != "remove" {
        crate::logging::error(format!("gsettings-strv invalid action={action}"));
        eprintln!("Usage: rm-monitor gsettings-strv append|remove <value>");
        return Err(2);
    }
    let current = env::var("CURRENT").ok();
    if action == "remove" && current.as_deref().unwrap_or("").trim().is_empty() {
        crate::logging::error("gsettings-strv remove requires CURRENT");
        eprintln!("gsettings-strv: remove requires CURRENT (refusing to invent [])");
        return Err(1);
    }
    let result = if action == "append" {
        append_strv(current.as_deref(), value)
    } else {
        remove_strv(current.as_deref(), value)
    };
    let result = match result {
        Ok(list) => list,
        Err(msg) => {
            crate::logging::error(format!("gsettings-strv {msg}"));
            eprintln!("gsettings-strv: {msg}");
            return Err(1);
        }
    };
    let formatted = format_strv(&result);
    crate::logging::info(format!(
        "gsettings-strv {action} -> {} entr(y/ies)",
        result.len()
    ));
    println!("{formatted}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_dedupes() {
        assert_eq!(
            append_strv(Some("['a']"), "a").unwrap(),
            vec!["a".to_string()]
        );
        assert_eq!(
            append_strv(Some("['a']"), "b").unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn remove_drops_all_matches() {
        assert_eq!(
            remove_strv(Some("['a', 'b', 'a']"), "a").unwrap(),
            vec!["b".to_string()]
        );
    }

    #[test]
    fn unclosed_quotes_are_rejected() {
        assert!(try_parse_strv(Some("['a]")).is_err());
        assert!(try_parse_strv(Some(r#"["unterminated]"#)).is_err());
    }

    #[test]
    fn utf8_values_round_trip() {
        let parsed = try_parse_strv(Some("@as ['café']")).unwrap();
        assert_eq!(parsed, vec!["café".to_string()]);
        assert_eq!(format_strv(&parsed), "['café']");
    }

    #[test]
    fn empty_forms_parse() {
        assert_eq!(try_parse_strv(None).unwrap(), Vec::<String>::new());
        assert_eq!(try_parse_strv(Some("")).unwrap(), Vec::<String>::new());
        assert_eq!(try_parse_strv(Some("[]")).unwrap(), Vec::<String>::new());
        assert_eq!(
            try_parse_strv(Some("@as []")).unwrap(),
            Vec::<String>::new()
        );
    }
}
