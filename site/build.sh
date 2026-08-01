#!/usr/bin/env bash
# Assembles the published site into site/build/:
#
#   site/build/index.html      the hand-written landing page
#   site/build/docs/**         mdBook over the *same* docs/ the repo serves
#   site/build/assets/         the demo GIF the landing page shows
#
# The docs are built from docs/ in place — no copies to drift — except for
# CHANGELOG.md, which lives at the repo root because that is where GitHub
# looks for it. mdBook cannot reach outside its src dir, so it is staged
# in and removed again (it is gitignored, so a failed run cannot leave the
# tree dirty in a way that gets committed). CONTRIBUTING.md deliberately
# stays out of the book: it links to AGENTS.md, CODE_OF_CONDUCT.md and
# issue labels, all of which only resolve on GitHub.
#
# Usage: site/build.sh          (needs mdbook on PATH)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

staged=(docs/CHANGELOG.md)
cleanup() { rm -f "${staged[@]}"; }
trap cleanup EXIT

cp CHANGELOG.md docs/CHANGELOG.md

rm -rf site/build
mdbook build

# The landing page and its assets sit beside the generated docs.
cp site/index.html site/style.css site/build/
mkdir -p site/build/assets
cp assets/demo.gif assets/social-card.png site/build/assets/

echo "site/build ready:"
find site/build -maxdepth 2 -name '*.html' | head -5
echo "  ($(find site/build -type f | wc -l | tr -d ' ') files)"
