#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Checks that every git-tracked *.rs and *.sh file in the repo starts with
# the correct SPDX license header:
#   - *.rs files: the header must be the file's first line, exactly
#       // SPDX-License-Identifier: GPL-3.0-or-later
#   - *.sh files: the header must be the first or second line (a shebang is
#     allowed on line 1), exactly
#       # SPDX-License-Identifier: GPL-3.0-or-later
#
# Usage: check-spdx.sh [repo-root]
# repo-root defaults to the current directory. When repo-root is a git
# working copy, tracked files are listed with `git ls-files`. When it is
# not (e.g. a Nix-sandboxed copy of the flake source, which already
# contains only git-tracked files), all *.rs/*.sh files under it are
# checked instead.
#
# Exits 0 with no output if every file passes. Otherwise prints the
# offending paths (one per line) and exits non-zero.

set -euo pipefail

root="${1:-.}"

rs_expected='// SPDX-License-Identifier: GPL-3.0-or-later'
sh_expected='# SPDX-License-Identifier: GPL-3.0-or-later'

list_files() {
  if git -C "$root" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git -C "$root" ls-files -- '*.rs' '*.sh'
  else
    find "$root" -type f \( -name '*.rs' -o -name '*.sh' \) -printf '%P\n' | sort
  fi
}

offenders=()

while IFS= read -r rel; do
  [ -z "$rel" ] && continue
  file="$root/$rel"

  first_line=""
  second_line=""
  { IFS= read -r first_line || true; IFS= read -r second_line || true; } < "$file"

  case "$rel" in
    *.rs)
      if [ "$first_line" != "$rs_expected" ]; then
        offenders+=("$rel")
      fi
      ;;
    *.sh)
      if [ "$first_line" != "$sh_expected" ] && [ "$second_line" != "$sh_expected" ]; then
        offenders+=("$rel")
      fi
      ;;
  esac
done < <(list_files)

if [ "${#offenders[@]}" -gt 0 ]; then
  printf '%s\n' "${offenders[@]}"
  exit 1
fi

exit 0
