#!/usr/bin/env bash
# Checks out Ace's web app at the commit the comparison was measured against.
set -euo pipefail
ACE_COMMIT="${ACE_COMMIT:-2502a66}"
dir="$(dirname "$0")/../.ace"
if [ ! -d "$dir/.git" ]; then git clone --filter=blob:none --no-checkout https://github.com/arpan404/ace "$dir"; fi
git -C "$dir" fetch --quiet origin
git -C "$dir" checkout --quiet "$ACE_COMMIT"
git -C "$dir" log --oneline -1
