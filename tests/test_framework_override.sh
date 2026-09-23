#!/usr/bin/env bash
# Verify missing local framework paths select the standalone component path
# without fetching executable framework code unless a ref is opted into.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf -- "$scratch"' EXIT
mkdir -p "$scratch/bin" "$scratch/home" "$scratch/isolated-repo"
# A live checkout may have the shared framework as its sibling, whereas a
# worktree does not. Copy only the installer files needed by read-only commands
# so the absent-sibling cases mean the same thing in both locations.
cp -a "$repo_root/install.sh" "$repo_root/lib" "$repo_root/installer" \
  "$scratch/isolated-repo/"
test_install="$scratch/isolated-repo/install.sh"

cat >"$scratch/bin/curl" <<'EOF'
#!/usr/bin/env bash
printf 'called\n' >"$FRAMEWORK_CURL_MARKER"
exit 1
EOF
chmod +x "$scratch/bin/curl"

output="$(
  HOME="$scratch/home" \
    PATH="$scratch/bin:$PATH" \
    ISC_FUNCTIONS_DIR="$scratch/missing-framework" \
    ISC_FUNCTIONS_REF= \
    FRAMEWORK_CURL_MARKER="$scratch/curl-called" \
    bash "$test_install" --list-components 2>"$scratch/fallback-stderr"
)"

if [ -e "$scratch/curl-called" ]; then
  printf 'explicit ISC_FUNCTIONS_DIR triggered a framework download\n' >&2
  exit 1
fi
if [[ "$output" != *$'rm_refresh_interval\tResource Monitor update time\ton'* ]]; then
  printf 'standalone component listing is missing\n' >&2
  exit 1
fi

output="$(
  HOME="$scratch/home" \
    PATH="$scratch/bin:$PATH" \
    ISC_FUNCTIONS_DIR= \
    ISC_FUNCTIONS_REF= \
    FRAMEWORK_CURL_MARKER="$scratch/curl-called" \
    bash "$test_install" --list-components 2>"$scratch/fallback-stderr"
)"

if [ -e "$scratch/curl-called" ]; then
  printf 'default component listing triggered a framework download\n' >&2
  exit 1
fi
if [[ "$output" != *$'rm_refresh_interval\tResource Monitor update time\ton'* ]]; then
  printf 'default standalone component listing is missing\n' >&2
  exit 1
fi

# The documented explicit ref still attempts a remote load. Our curl stub
# fails, so the installer must then use the same standalone listing.
output="$(
  HOME="$scratch/home" \
    PATH="$scratch/bin:$PATH" \
    ISC_FUNCTIONS_DIR= \
    ISC_FUNCTIONS_REF=master \
    FRAMEWORK_CURL_MARKER="$scratch/curl-called" \
    bash "$test_install" --list-components 2>"$scratch/fallback-stderr"
)"
if [ ! -e "$scratch/curl-called" ]; then
  printf 'explicit ISC_FUNCTIONS_REF did not attempt a framework download\n' >&2
  exit 1
fi
if [[ "$output" != *$'rm_refresh_interval\tResource Monitor update time\ton'* ]]; then
  printf 'fallback listing after failed opted-in download is missing\n' >&2
  exit 1
fi
