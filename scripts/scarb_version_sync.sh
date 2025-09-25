#!/usr/bin/env bash
#
# Sync or verify [package].version and [workspace.package].version in all Scarb.toml files.
#
# Usage:
#   scripts/scarb_version_sync.sh --version 1.7.0-alpha.3            # update files in-place
#   scripts/scarb_version_sync.sh -v 1.7.0-alpha.3 --check           # verify only (no changes)
#   scripts/scarb_version_sync.sh -v 1.7.0-alpha.3 --root path/to/dir
#
# Notes:
# - Updates the `version = "..."` line inside both `[package]` and `[workspace.package]` sections.
# - Ignores `target/` and `.git/`.
# - Works on macOS (BSD awk) and Linux (GNU awk/mawk).

set -uo pipefail

ROOT="."
CHECK=0
VERSION=""
VERBOSE=0

die() { echo "error: $*" >&2; exit 2; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    -v|--version) VERSION="${2:-}"; shift 2 ;;
    --check)      CHECK=1; shift ;;
    --root)       ROOT="${2:-}"; shift 2 ;;
    -q|--quiet)   VERBOSE=0; shift ;;
    -V|--verbose) VERBOSE=1; shift ;;
    -h|--help)
      sed -n '1,30p' "$0"
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

[[ -n "$VERSION" ]] || die "--version is required"

# Finder
find_scarb() {
  # -print0 so we safely handle weird paths
  # Exclude crates/dojo/macros/Scarb.toml as it's managed independently.
  find "$ROOT" -type d \( -name .git -o -name target \) -prune -o \
       -path "*/crates/dojo/macros/Scarb.toml" -prune -o \
       -path "*/crates/dojo/macros/Scarb.lock" -prune -o \
       -type f -name Scarb.toml -print0
}

# Verify a single file's [package].version and [workspace.package].version matches $VERSION
check_file() {
  local file="$1"
  # awk logic:
  # - toggle in_pkg when entering/leaving sections
  # - if inside [package] or [workspace.package] and see a version= line, compare its quoted value
  awk -v ver="$VERSION" -v file="$file" '
    BEGIN { in_pkg=0; ok=1 }
    /^[[:space:]]*\[package\][[:space:]]*$/ { in_pkg=1; next }
    /^[[:space:]]*\[workspace\.package\][[:space:]]*$/ { in_pkg=1; next }
    /^[[:space:]]*\[[^]]+\][[:space:]]*$/ && $0 !~ /^\[(package|workspace\.package)\]/ { in_pkg=0 }
    in_pkg && /^[[:space:]]*version[[:space:]]*=/ {
      if (match($0, /"[^\"]+"/)) {
        v = substr($0, RSTART+1, RLENGTH-2)
        if (v != ver) {
          printf("Mismatch in %s: found \"%s\" (expected \"%s\")\n", file, v, ver) > "/dev/stderr"
          ok=0
        }
      } else {
        printf("Malformed version line in %s: %s\n", file, $0) > "/dev/stderr"
        ok=0
      }
    }
    END { exit ok?0:1 }
  ' "$file"
}

# Update a single file in-place (version lines inside [package] and [workspace.package])
update_file() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"

  # Replace the quoted version string on the version= line (inside [package] and [workspace.package]).
  # This keeps things simple and portable. Trailing comments on that line are removed.
  awk -v ver="$VERSION" '
    BEGIN { in_pkg=0 }
    /^[[:space:]]*\[package\][[:space:]]*$/ { in_pkg=1; print; next }
    /^[[:space:]]*\[workspace\.package\][[:space:]]*$/ { in_pkg=1; print; next }
    /^[[:space:]]*\[[^]]+\][[:space:]]*$/ && $0 !~ /^\[(package|workspace\.package)\]/ { in_pkg=0; print; next }
    {
      if (in_pkg && $0 ~ /^[[:space:]]*version[[:space:]]*=/) {
        sub(/^[[:space:]]*version[[:space:]]*=[[:space:]]*".*"/, "version = \"" ver "\"")
        print
      } else {
        print
      }
    }
  ' "$file" > "$tmp"

  if ! cmp -s "$file" "$tmp"; then
    mv "$tmp" "$file"
    return 1  # changed
  else
    rm -f "$tmp"
    return 0  # unchanged
  fi
}

# Main
changed=0
found=0
fail=0

while IFS= read -r -d '' f; do
  found=$((found+1))
  if [[ "$CHECK" -eq 1 ]]; then
    if ! check_file "$f"; then
      fail=1
    elif [[ "$VERBOSE" -eq 1 ]]; then
      echo "OK: $f"
    fi
  else
    if update_file "$f"; then
      # unchanged
      [[ "$VERBOSE" -eq 1 ]] && echo "unchanged: $f"
    else
      changed=$((changed+1))
      echo "updated:   $f"
    fi
  fi
done < <(find_scarb)

if [[ "$found" -eq 0 ]]; then
  echo "No Scarb.toml files found under $ROOT"
fi

if [[ "$CHECK" -eq 1 ]]; then
  if [[ "$fail" -ne 0 ]]; then
    echo "Scarb version check failed." >&2
    exit 1
  fi
  echo "All Scarb.toml [package].version and [workspace.package].version match $VERSION"
fi

exit 0
