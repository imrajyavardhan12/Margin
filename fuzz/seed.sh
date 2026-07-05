#!/bin/sh
# Seed the fuzz corpora from the parser test suite. Used by both CI
# (.github/workflows/fuzz.yml) and developers before a local run:
#
#   ./fuzz/seed.sh && cargo +nightly fuzz run parse_unified
#
# The corpus directories are gitignored; grown entries survive alongside
# the refreshed seeds (CI additionally persists them via actions/cache).
# The intraline target takes Arbitrary-encoded string pairs, not patches,
# so it starts from libFuzzer's own corpus.
set -eu
cd "$(dirname "$0")"
for target in parse_unified strip_ansi; do
    mkdir -p "corpus/$target"
    cp ../crates/margin-core/tests/corpus/*.patch "corpus/$target/"
done
echo "seeded: parse_unified strip_ansi"
