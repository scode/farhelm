#!/usr/bin/env bash
# Build a throwaway checkout for evaluating the deflake tooling end to end.
#
# The eval answers one question: can an agent that has never seen this
# tooling, given only "start a deflake run" and deflake/AGENTS.md, drive a
# sweep to completion, record a flake, fix an easy deterministic failure, and
# never poll? Answering it on the real suite would take hours and would need
# a real flake to appear on cue, so this script makes a copy of the current
# checkout whose `main` carries two planted fixtures, and points a local bare
# repository at it as `origin` so pushes and fetches are harmless. The agent
# then runs `deflake start --profile eval`, which runs narrow, fast batteries
# (see `phases("eval")` in deflake/bin/deflake). deflake/EVAL.md is the
# procedure around this script.
#
# What gets planted, and what the sweep is expected to do with each:
#   - a farhelm-proto unit test that fails exactly once per lifetime of a
#     marker file under /tmp, then passes: the nextest phase must report it
#     with verdict `flake` after three passing reruns.
#   - a JS unit test with a wrong assertion: the js-unit phase must report the
#     whole battery with verdict `deterministic` after three failing reruns.
#
# Usage: deflake/eval-setup.sh <empty-or-missing directory>
# Prints the checkout path to use as the agent's working directory. Run from
# anywhere inside the source checkout. The fixed marker path is acceptable
# here only because the fixture lives in a throwaway commit that is never
# merged; do not copy the pattern into a real test.

set -u

dest=${1:?usage: deflake/eval-setup.sh <directory>}
source_checkout=$(git rev-parse --show-toplevel) || exit 1
base_ref=$(git -C "$source_checkout" rev-parse HEAD) || exit 1
marker=/tmp/deflake-eval-flaky-marker

test ! -e "$dest" || test -z "$(ls -A "$dest")" || { echo "eval-setup: $dest is not empty" >&2; exit 1; }
mkdir -p "$dest" || exit 1
dest=$(cd "$dest" && pwd -P) || exit 1

# A bare "origin" so the agent's `jj git fetch` and any push have somewhere
# harmless to go; the real remote must never see eval commits.
git init -q --bare "$dest/remote.git" || exit 1
git clone -q "$source_checkout" "$dest/checkout" || exit 1
cd "$dest/checkout" || exit 1
git checkout -q --detach "$base_ref" || exit 1
git branch -f main "$base_ref" || exit 1
git remote set-url origin "$dest/remote.git" || exit 1
git push -q origin main || exit 1

# jj on top of the clone, with the identity the source checkout uses; without
# it every commit the agent makes carries an empty author.
jj git init --colocate >/dev/null 2>&1 || exit 1
jj config set --repo user.name "$(git -C "$source_checkout" config user.name)" 2>/dev/null || exit 1
jj config set --repo user.email "$(git -C "$source_checkout" config user.email)" 2>/dev/null || exit 1
jj bookmark track main@origin >/dev/null 2>&1 || exit 1
jj new main >/dev/null 2>&1 || exit 1

cat > crates/farhelm-proto/src/deflake_eval.rs <<'EOF'
//! Deflake-eval fixture: a test that fails exactly once per marker lifetime.
//!
//! The first run finds no marker, creates it, and fails; every later run sees
//! the marker and passes. That is the shape of a flake the sweep should
//! classify as one. Throwaway commit; never merged.

#[cfg(test)]
mod tests {
    #[test]
    fn deflake_eval_flaky_fails_once_then_passes() {
        let marker = std::path::Path::new("/tmp/deflake-eval-flaky-marker");
        if marker.exists() {
            return;
        }
        std::fs::write(marker, b"seen").unwrap();
        panic!("deflake eval: first run fails on purpose");
    }
}
EOF
printf '\n// Deflake-eval fixture module; throwaway.\nmod deflake_eval;\n' >> crates/farhelm-proto/src/lib.rs || exit 1
cat > crates/farhelm-ui/js-tests/zz-deflake-eval.test.js <<'EOF'
// Deflake-eval fixture: a deterministic failure. Throwaway; never merged.
const test = require("node:test");
const assert = require("node:assert");
test("deflake eval: the answer is forty-two", () => {
  assert.strictEqual(40 + 1, 42);
});
EOF

jj commit -m "test: plant deflake eval fixtures (throwaway)" >/dev/null 2>&1 || exit 1
jj bookmark set main -r @- >/dev/null 2>&1 || exit 1
jj git push --bookmark main >/dev/null 2>&1 || exit 1
rm -f "$marker"

echo "$dest/checkout"
