#!/usr/bin/env bash
# Assembles the published site into site/build/:
#
#   site/build/index.html      the hand-written landing page
#   site/build/docs/**         mdBook over the *same* docs/ the repo serves
#   site/build/assets/         the demo GIF the landing page shows
#
# The docs are built from docs/ in place — no copies to drift — except for
# CHANGELOG.md and CONTRIBUTING.md, which live at the repo root because
# that is where GitHub looks for them. mdBook cannot reach outside its src
# dir, so those two are staged in and removed again (they are gitignored,
# so a failed run cannot leave the tree dirty in a way that gets committed).
#
# Usage: site/build.sh          (needs mdbook on PATH)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

staged=(docs/CHANGELOG.md docs/CONTRIBUTING.md)
cleanup() { rm -f "${staged[@]}"; }
trap cleanup EXIT

cp CHANGELOG.md docs/CHANGELOG.md
cp CONTRIBUTING.md docs/CONTRIBUTING.md

rm -rf site/build
mdbook build

# The landing page and its assets sit beside the generated docs.
cp site/index.html site/style.css site/build/
mkdir -p site/build/assets
cp assets/demo.gif assets/social-card.png site/build/assets/

echo "site/build ready:"
find site/build -maxdepth 2 -name '*.html' | head -5
echo "  ($(find site/build -type f | wc -l | tr -d ' ') files)"
