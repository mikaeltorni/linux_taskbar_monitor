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
# Depends on: msg, ISC_COMPONENTS (after installer/components.sh is sourced),
#             ISC_REPO_NAME, ISC_REPO_LABEL, ISC_POSTFLIGHT (optional),
#             TARGET_HOME, and the install/detect/uninstall functions named in
#             the manifest.

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
  mkdir -p "$dir" 2>/dev/null || return 0
  printf '%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || true)" >"$dir/$id" 2>/dev/null || true
}

_sc_clear_receipt() {
  rm -f "$(_sc_receipt_dir)/$1" 2>/dev/null || true
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
    ran=$((ran + 1))
    if [[ -z "$fn" ]] || ! declare -F "$fn" >/dev/null 2>&1; then
      msg "WARN: install function for '$id' is missing — skipping"
      failed+=("$id")
      continue
    fi
    msg "[${ISC_REPO_NAME:-installer}] Installing component: $label ($id)"
    if ! "$fn"; then
      msg "WARN: component '$id' failed (continuing)"
      failed+=("$id")
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
    msg "[${ISC_REPO_NAME:-installer}] Components with failures: ${failed[*]}"
    return 1
  fi
  return 0
}

# _sc_uninstall_selected IDS — Reverse-order uninstall when a handler exists.
_sc_uninstall_selected() {
  local selected=" $(tr ',\n' '  ' <<<"$1") "
  local -a order=()
  local entry id i un label
  for entry in "${ISC_COMPONENTS[@]}"; do
    IFS='|' read -r id _ <<<"$entry"
    order+=("$id")
  done
  for (( i=${#order[@]}-1; i>=0; i-- )); do
    id="${order[$i]}"
    [[ "$selected" == *" $id "* ]] || continue
    label="$(_sc_field "$id" 1)"
    un="$(_sc_field "$id" 5)"
    if [[ -z "$un" ]]; then
      msg "[${ISC_REPO_NAME:-installer}] '$label' ($id) has no uninstall step — clearing receipt only."
      _sc_clear_receipt "$id"
      continue
    fi
    if ! declare -F "$un" >/dev/null 2>&1; then
      msg "WARN: uninstall function '$un' for '$id' is not defined — skipping"
      continue
    fi
    msg "[${ISC_REPO_NAME:-installer}] Uninstalling component: $label ($id)"
    if "$un"; then
      _sc_clear_receipt "$id"
    else
      msg "WARN: uninstall of '$id' failed (continuing)"
    fi
  done
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
    msg "ERROR: ISC_COMPONENTS manifest is not defined."
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
    --list-configurable-components|--list-select-configure-components|--list-component-config-values)
      # Full configurator discovery needs the framework; empty stdout is valid
      # for the master orchestrator when nothing is configurable here.
      return 0
      ;;
    --configure-component|--configure-component=*|--export-selection)
      msg "ERROR: $mode requires the linux_installation_scripts_functions framework."
      msg "       Clone it as a sibling or set ISC_FUNCTIONS_DIR, then re-run."
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
      [[ -n "$ids" ]] || { msg "ERROR: --uninstall needs component ids"; return 1; }
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
      [[ -n "$ids" ]] || { msg "ERROR: --reconfigure needs component ids"; return 1; }
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
      [[ -n "$ids" ]] || { msg "ERROR: --select needs component ids"; return 1; }
      _sc_run_selected "$ids"
      return $?
      ;;
    *)
      msg "ERROR: unknown argument '$mode' (standalone fallback)."
      _sc_usage >&2
      return 1
      ;;
  esac
}
