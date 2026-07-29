//! GSettings string-array helpers for the installer (`gsettings-strv` CLI).

use std::collections::HashSet;
use std::env;

/// Parse `gsettings get` string-array output into normalized values.
///
/// # Parameters
/// - `raw`: Optional `gsettings get` output (`@as [...]` or `[...]`). `None`
///   and empty strings yield an empty list.
pub fn parse_strv(raw: Option<&str>) -> Vec<String> {
    let mut value = raw.unwrap_or("").trim().to_string();
    if let Some(rest) = value.strip_prefix("@as ") {
        value = rest.trim().to_string();
    }
    if value.is_empty() {
        return Vec::new();
    }
    pythonish_list(&value).unwrap_or_default()
}

/// Parse a Python/gsettings list literal like `['a', "b"]`.
fn pythonish_list(value: &str) -> Option<Vec<String>> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return None;
    }
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }

    let bytes = inner.as_bytes();
    let mut items = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && ((bytes[i] as char).is_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let quote = bytes[i];
        if quote != b'\'' && quote != b'"' {
            return None;
        }
        i += 1;
        let mut out = String::new();
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if bytes[i] == quote {
                i += 1;
                break;
            }
            out.push(bytes[i] as char);
            i += 1;
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
pub fn append_strv(raw: Option<&str>, value: &str) -> Vec<String> {
    let mut current = unique_values(parse_strv(raw));
    if !value.is_empty() && !current.iter().any(|v| v == value) {
        current.push(value.to_string());
    }
    current
}

/// Remove every occurrence of `value`.
///
/// # Parameters
/// - `raw`: Current `gsettings get` string-array text (or `None`).
/// - `value`: Entry to drop.
pub fn remove_strv(raw: Option<&str>, value: &str) -> Vec<String> {
    unique_values(
        parse_strv(raw)
            .into_iter()
            .filter(|item| item != value)
            .collect(),
    )
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
/// action (usage error). Reads `CURRENT` for the prior `gsettings get` text.
pub fn run(action: &str, value: &str) -> Result<(), i32> {
    crate::logging::info(format!("gsettings-strv {action} value={value}"));
    if action != "append" && action != "remove" {
        crate::logging::error(format!("gsettings-strv invalid action={action}"));
        eprintln!("Usage: rm-monitor gsettings-strv append|remove <value>");
        return Err(2);
    }
    let current = env::var("CURRENT").ok();
    let result = if action == "append" {
        append_strv(current.as_deref(), value)
    } else {
        remove_strv(current.as_deref(), value)
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
    fn parse_and_format_round_trip_shape() {
        let parsed = parse_strv(Some("['a', 'b']"));
        assert_eq!(parsed, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(format_strv(&parsed), "['a', 'b']");
    }

    #[test]
    fn append_dedupes() {
        assert_eq!(append_strv(Some("['a']"), "a"), vec!["a".to_string()]);
        assert_eq!(
            append_strv(Some("['a']"), "b"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn strips_as_prefix() {
        assert_eq!(parse_strv(Some("@as ['x']")), vec!["x".to_string()]);
    }
}
