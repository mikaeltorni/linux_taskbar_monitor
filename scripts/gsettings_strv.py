"""gsettings_strv.py - helpers for GNOME gsettings string-array values.

Components:
  - parse_strv: Parse gsettings array output into a list of strings.
  - append_strv: Return a de-duplicated array with a value appended.
  - remove_strv: Return a de-duplicated array with a value removed.
  - format_strv: Serialize values back to the gsettings string-array format.
"""

from __future__ import annotations

import ast
import os
import sys

from rm_logging import LOGGER, log_call


def parse_strv(raw: str | None) -> list[str]:
    """Parse gsettings string-array output into normalized string values.

    Tolerates the optional ``@as`` type-annotation prefix that ``gsettings get``
    emits and returns an empty list for any value that is missing, empty, not a
    valid Python literal, or not a list.

    Args:
        raw: Raw ``gsettings get`` output for a string-array key, or ``None``.

    Returns:
        The parsed values as a list of strings, or an empty list when ``raw`` is
        absent or cannot be parsed as a list.
    """
    value = (raw or "").strip()
    if value.startswith("@as "):
        value = value[4:].strip()

    try:
        parsed = ast.literal_eval(value) if value else []
    except (SyntaxError, ValueError):
        return []

    if not isinstance(parsed, list):
        return []

    return [str(item) for item in parsed]


def unique_values(values: list[str]) -> list[str]:
    """Return values with duplicates removed while preserving first appearance.

    Args:
        values: Values to de-duplicate.

    Returns:
        A new list containing each distinct value once, in first-seen order.
    """
    return list(dict.fromkeys(values))


@log_call(LOGGER)
def append_strv(raw: str | None, value: str) -> list[str]:
    """Return normalized values with ``value`` appended if it is not present.

    Args:
        raw: Raw ``gsettings get`` output for the current string-array value.
        value: Value to append. Empty strings and existing values are ignored.

    Returns:
        The de-duplicated current values, with ``value`` appended when it was
        non-empty and not already present.
    """
    current = unique_values(parse_strv(raw))
    if value and value not in current:
        current.append(value)
    return current


@log_call(LOGGER)
def remove_strv(raw: str | None, value: str) -> list[str]:
    """Return normalized values with every occurrence of ``value`` removed.

    Args:
        raw: Raw ``gsettings get`` output for the current string-array value.
        value: Value to remove from the array.

    Returns:
        The de-duplicated current values with every occurrence of ``value`` removed.
    """
    return unique_values([item for item in parse_strv(raw) if item != value])


def format_strv(values: list[str]) -> str:
    """Serialize values for ``gsettings set``.

    Args:
        values: Values to serialize.

    Returns:
        A bracketed, comma-separated array of single-quoted strings accepted by
        ``gsettings set`` for an ``as`` (string-array) key.
    """
    return "[" + ", ".join(repr(str(item)) for item in values) + "]"


@log_call(LOGGER)
def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint used by ``install.sh``.

    Reads the current array from the ``CURRENT`` environment variable, applies
    the requested ``append`` or ``remove`` action, and prints the resulting
    array (via :func:`format_strv`) to stdout.

    Args:
        argv: Argument list ``[action, value]`` where ``action`` is ``append`` or
            ``remove``. Defaults to ``sys.argv[1:]`` when ``None``.

    Returns:
        ``0`` on success, or ``2`` on a usage error (wrong argument count or an
        unknown action).
    """
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 2 or args[0] not in {"append", "remove"}:
        LOGGER.error("Invalid gsettings_strv usage: %r", args)
        print("Usage: gsettings_strv.py append|remove <value>", file=sys.stderr)
        return 2

    action, value = args
    current = os.environ.get("CURRENT", "")
    result = append_strv(current, value) if action == "append" else remove_strv(current, value)
    print(format_strv(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
