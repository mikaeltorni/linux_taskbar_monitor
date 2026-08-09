#!/usr/bin/env bash
# standalone_component_fallback.sh — Minimal component_main when the shared
# linux_installation_scripts_functions framework cannot be loaded (private
# GitHub raw URL, offline clone without sibling, etc.).
#
# Sourced by install.sh only after sibling + curl activation both fail. Keeps
# this repository a working standalone installer for the core Resource Monitor
# path and default/select/list/detect/uninstall/reconfigure contracts that the
# master orchestrator and README document. Interactive TTY menus and
# installation_config JSON export still require the full framework.
#
# Depends on: msg, run_as_target, ISC_COMPONENTS (after installer/components.sh
# is sourced), ISC_REPO_NAME, ISC_REPO_LABEL, ISC_PREFLIGHT / ISC_POSTFLIGHT
# (optional), TARGET_HOME, and the install/detect/uninstall functions named in
# the manifest.

# Framework-compatible aliases so patch helpers that call _isc_mark_installed
# keep working when only the fallback is loaded.
_isc_mark_installed() { _sc_mark_installed "$@"; }
_isc_clear_receipt() { _sc_clear_receipt "$@"; }

# _sc_field ID INDEX — Print pipe-field INDEX (0-based) for component ID.
_sc_field() {
  local want="$1" idx="$2" entry id
  for entry in "${ISC_COMPONENTS[@]}"; do
    IFS='|' read -r id _ <<<"$entry"
    [[ "$id" == "$want" ]] || continue
    IFS='|' read -r -a fields <<<"$entry"
    printf '%s\n' "${fields[$idx]:-}"
    return 0
  done
  return 1
}

# _sc_all_ids — Print every component id in manifest order.
_sc_all_ids() {
  local entry id
  for entry in "${ISC_COMPONENTS[@]}"; do
    IFS='|' read -r id _ <<<"$entry"
    [[ -n "$id" ]] && printf '%s\n' "$id"
  done
}

# _sc_default_ids — Print default-on component ids.
_sc_default_ids() {
  local entry id label def
  for entry in "${ISC_COMPONENTS[@]}"; do
    IFS='|' read -r id label def _ <<<"$entry"
    [[ "$def" == "on" ]] && printf '%s\n' "$id"
  done
}

# _sc_is_installed ID — Prefer detect_fn; else receipt under XDG state.
_sc_receipt_dir() {
  local state_home="${XDG_STATE_HOME:-${TARGET_HOME:-$HOME}/.local/state}"
  printf '%s/isc/receipts/%s\n' "$state_home" "${ISC_REPO_NAME:-installer}"
}

_sc_mark_installed() {
  local id="$1" dir
  dir="$(_sc_receipt_dir)"
  if declare -F run_as_target >/dev/null 2>&1; then
    run_as_target mkdir -p "$dir" 2>/dev/null || return 0
    printf '%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || true)" \
      | run_as_target tee "$dir/$id" >/dev/null 2>/dev/null || true
  else
    mkdir -p "$dir" 2>/dev/null || return 0
    printf '%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || true)" >"$dir/$id" 2>/dev/null || true
  fi
}

_sc_clear_receipt() {
  local path
  path="$(_sc_receipt_dir)/$1"
  if declare -F run_as_target >/dev/null 2>&1; then
    run_as_target rm -f "$path" 2>/dev/null || true
  else
    rm -f "$path" 2>/dev/null || true
  fi
}

_sc_is_installed() {
  local id="$1" detect
  detect="$(_sc_field "$id" 4)"
  if [[ -n "$detect" ]] && declare -F "$detect" >/dev/null 2>&1; then
    "$detect"
    return $?
  fi
  [[ -f "$(_sc_receipt_dir)/$id" ]]
}

# _sc_run_selected IDS — Install whitespace/comma-separated component ids.
_sc_run_selected() {
  local selected=" $(tr ',\n' '  ' <<<"$1") "
  local entry id label def fn ran=0
  local -a failed=()
  for entry in "${ISC_COMPONENTS[@]}"; do
    IFS='|' read -r id label def fn _ <<<"$entry"
    [[ "$selected" == *" $id "* ]] || continue
    if (( ran == 0 )) && [[ -n "${ISC_PREFLIGHT:-}" ]] \
       && declare -F "$ISC_PREFLIGHT" >/dev/null 2>&1; then
      if ! "$ISC_PREFLIGHT"; then
        msg "ERROR: preflight '$ISC_PREFLIGHT' failed" >&2
        return 1
      fi
    fi
    ran=$((ran + 1))
    if [[ -z "$fn" ]] || ! declare -F "$fn" >/dev/null 2>&1; then
      msg "WARN: install function for '$id' is missing — skipping" >&2
      failed+=("$id")
      continue
    fi
    msg "[${ISC_REPO_NAME:-installer}] Installing component: $label ($id)"
    if ! "$fn"; then
      msg "ERROR: component '$id' failed" >&2
      failed+=("$id")
      msg "[${ISC_REPO_NAME:-installer}] Components with failures: ${failed[*]}" >&2
      return 1
    else
      _sc_mark_installed "$id"
    fi
  done
  if (( ran > 0 )) && [[ -n "${ISC_POSTFLIGHT:-}" ]] \
     && declare -F "$ISC_POSTFLIGHT" >/dev/null 2>&1; then
    "$ISC_POSTFLIGHT" || true
  fi
  if (( ran == 0 )); then
    msg "[${ISC_REPO_NAME:-installer}] No components selected — nothing to do."
  fi
  if (( ${#failed[@]} > 0 )); then
    msg "[${ISC_REPO_NAME:-installer}] Components with failures: ${failed[*]}" >&2
    return 1
  fi
  return 0
}

# _sc_uninstall_selected IDS — Reverse-order uninstall when a handler exists.
_sc_uninstall_selected() {
  local selected=" $(tr ',\n' '  ' <<<"$1") "
  local -a order=() failed=()
  local entry id i un label ran=0
  for entry in "${ISC_COMPONENTS[@]}"; do
    IFS='|' read -r id _ <<<"$entry"
    order+=("$id")
  done
  for (( i=${#order[@]}-1; i>=0; i-- )); do
    id="${order[$i]}"
    [[ "$selected" == *" $id "* ]] || continue
    label="$(_sc_field "$id" 1)"
    un="$(_sc_field "$id" 5)"
    ran=$((ran + 1))
    if [[ -z "$un" ]]; then
      # Live detect_fn means receipt-only uninstall would lie to --detect.
      detect="$(_sc_field "$id" 4)"
      if [[ -n "$detect" ]]; then
        msg "ERROR: '$label' ($id) has detect but no uninstall; refusing to clear receipt." >&2
        failed+=("$id")
        continue
      fi
      msg "[${ISC_REPO_NAME:-installer}] '$label' ($id) has no uninstall step — clearing receipt only." >&2
      _sc_clear_receipt "$id"
      continue
    fi
    if ! declare -F "$un" >/dev/null 2>&1; then
      msg "WARN: uninstall function '$un' for '$id' is not defined — skipping" >&2
      failed+=("$id")
      continue
    fi
    msg "[${ISC_REPO_NAME:-installer}] Uninstalling component: $label ($id)"
    if "$un"; then
      _sc_clear_receipt "$id"
    else
      msg "WARN: uninstall of '$id' failed (continuing)" >&2
      failed+=("$id")
    fi
  done
  if (( ran == 0 )); then
    msg "[${ISC_REPO_NAME:-installer}] No components selected to uninstall — nothing to do."
  fi
  if (( ${#failed[@]} > 0 )); then
    msg "[${ISC_REPO_NAME:-installer}] Components with uninstall failures: ${failed[*]}" >&2
    return 1
  fi
  return 0
}

_sc_list_components() {
  local entry id label def
  for entry in "${ISC_COMPONENTS[@]}"; do
    IFS='|' read -r id label def _ <<<"$entry"
    printf '%s\t%s\t%s\n' "$id" "$label" "$def"
  done
}

_sc_detect_all() {
  local id
  while IFS= read -r id; do
    [[ -z "$id" ]] && continue
    if _sc_is_installed "$id"; then
      printf '%s\tinstalled\n' "$id"
    else
      printf '%s\tabsent\n' "$id"
    fi
  done < <(_sc_all_ids)
}

_sc_usage() {
  cat <<EOF
${ISC_REPO_LABEL:-${ISC_REPO_NAME:-installer}} — standalone component fallback

The full installer framework was unavailable; this built-in path supports the
core non-interactive contract. Clone linux_installation_scripts_functions as a
sibling (or set ISC_FUNCTIONS_DIR) for the interactive menu and config export.

Usage:
  bash install.sh                 Install default-on components (non-interactive)
  bash install.sh --default       Same as above
  bash install.sh --all           Install every component
  bash install.sh --select a,b    Install exactly these ids
  bash install.sh --list-components
  bash install.sh --detect
  bash install.sh --reconfigure a,b
  bash install.sh --uninstall a,b
  bash install.sh --help
EOF
}

# component_main — Framework-compatible entrypoint for the fallback path.
component_main() {
  local mode="${1:-}" ids
  if [[ -z "${ISC_COMPONENTS[*]:-}" ]]; then
    msg "ERROR: ISC_COMPONENTS manifest is not defined." >&2
    return 1
  fi

  case "$mode" in
    --help|-h)
      _sc_usage
      return 0
      ;;
    --list-components)
      _sc_list_components
      return 0
      ;;
    --list-configurable-components)
      local id
      while IFS= read -r id; do
        [[ -z "$id" ]] && continue
        [[ -n "$(_sc_field "$id" 8)" ]] && printf '%s\n' "$id"
      done < <(_sc_all_ids)
      return 0
      ;;
    --list-select-configure-components)
      local id
      while IFS= read -r id; do
        [[ -z "$id" ]] && continue
        [[ "$(_sc_field "$id" 10)" == "select_configure" ]] && printf '%s\n' "$id"
      done < <(_sc_all_ids)
      return 0
      ;;
    --list-component-config-values)
      local id status value
      while IFS= read -r id; do
        [[ -z "$id" ]] && continue
        status="$(_sc_field "$id" 9)"
        [[ -n "$status" ]] && declare -F "$status" >/dev/null 2>&1 || continue
        value="$("$status" 2>/dev/null || true)"
        [[ -n "$value" ]] && printf '%s\t%s\n' "$id" "$value"
      done < <(_sc_all_ids)
      return 0
      ;;
    --configure-component|--configure-component=*|--export-selection)
      msg "ERROR: $mode requires the linux_installation_scripts_functions framework." >&2
      msg "       Clone it as a sibling or set ISC_FUNCTIONS_DIR, then re-run." >&2
      return 1
      ;;
    --detect)
      _sc_detect_all
      return 0
      ;;
    --uninstall|--uninstall=*)
      if [[ "$mode" == --uninstall=* ]]; then
        ids="${mode#--uninstall=}"
      else
        shift
        ids="${1:-}"
      fi
      [[ -n "$ids" ]] || { msg "ERROR: --uninstall needs component ids" >&2; return 1; }
      _sc_uninstall_selected "$ids"
      return $?
      ;;
    --reconfigure|--reconfigure=*)
      if [[ "$mode" == --reconfigure=* ]]; then
        ids="${mode#--reconfigure=}"
      else
        shift
        ids="${1:-}"
      fi
      [[ -n "$ids" ]] || { msg "ERROR: --reconfigure needs component ids" >&2; return 1; }
      _sc_run_selected "$ids"
      return $?
      ;;
    --default|"")
      ids="$(_sc_default_ids | tr '\n' ' ')"
      _sc_run_selected "$ids"
      return $?
      ;;
    --all)
      ids="$(_sc_all_ids | tr '\n' ' ')"
      _sc_run_selected "$ids"
      return $?
      ;;
    --select|--select=*)
      if [[ "$mode" == --select=* ]]; then
        ids="${mode#--select=}"
      else
        shift
        ids="${1:-}"
      fi
      [[ -n "$ids" ]] || { msg "ERROR: --select needs component ids" >&2; return 1; }
      _sc_run_selected "$ids"
      return $?
      ;;
    *)
      msg "ERROR: unknown argument '$mode' (standalone fallback)." >&2
      _sc_usage >&2
      return 1
      ;;
  esac
}
