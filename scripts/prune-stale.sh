#!/usr/bin/env bash
# Makes the repo match the VeriChain manifest: lists (default) or removes (--apply) TRACKED files that are not in
# MANIFEST.txt. A zip can only add and overwrite files, never delete, so run this after unzipping an update.
#   ./scripts/prune-stale.sh            dry run: shows what would go, grouped by top-level folder
#   ./scripts/prune-stale.sh --apply    git rm those files (recoverable: nothing is committed; `git restore --staged --worktree .` undoes it)
# Never touches .git, untracked files, or ignored files (target/, data dirs). Untracked leftovers are only listed.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
M=MANIFEST.txt
[ -f "$M" ] || { echo "MANIFEST.txt not found in $(pwd)" >&2; exit 2; }
out="$(mktemp)"; trap 'rm -f "$out"' EXIT
git ls-files | grep -vxFf "$M" > "$out" || true
n="$(wc -l < "$out" | tr -d ' ')"
if [ "$n" = "0" ]; then echo "Nothing stale: every tracked file is in the manifest."; else
  echo "$n tracked file(s) are NOT in the manifest, by top-level folder:"
  awk -F/ '{ if (NF==1) print "(root)"; else print $1 }' "$out" | sort | uniq -c | sort -rn | sed 's/^/  /'
  echo; echo "First 40:"; head -40 "$out" | sed 's/^/  /'
fi
others="$(git ls-files --others --exclude-standard | grep -vxFf "$M" || true)"
[ -n "$others" ] && { echo; echo "Untracked files present (not touched):"; echo "$others" | head -20 | sed 's/^/  /'; }
if [ "${1:-}" = "--apply" ] && [ "$n" != "0" ]; then
  tr '\n' '\0' < "$out" | xargs -0 git rm -q --
  echo; echo "Removed $n file(s) from the working tree and index. Review with 'git status', then commit."
elif [ "$n" != "0" ]; then echo; echo "Dry run only. Re-run with --apply to remove them."; fi
