#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for destination in web/licenses platforms/android/markit/src/main/assets/markit-licenses platforms/ios/Sources/Markit/Licenses; do
  mkdir -p "$destination"
  cp -R licenses/. "$destination/"
  cp THIRD_PARTY_NOTICES.md "$destination/"
done
