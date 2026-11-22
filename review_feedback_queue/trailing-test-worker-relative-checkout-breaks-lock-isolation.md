# Passing --checkout . or .. to test-worker breaks worker-root derivation and lock isolation

Reviewed commit: 022d0f010fe5fb0d5acd3e73e02b0569517ec1bc

## TLDR

Passing `--checkout .` or `--checkout ..` to the test-worker script places the remote checkout state directly in the
worker's root or home directory, uses the wrong worker-wide lock files so bootstrap and tests can overlap, and forces a
full re-bootstrap on every run.

## Details

**other.** Source inspection. Main reference `scripts/test-worker/test-worker.sh:70`.

In `scripts/test-worker/test-worker.sh:61`, the default `checkout` is canonicalized via `cd ... && pwd`, but
`--checkout` at line 70 stores `$2` verbatim. Lines 137-138 compute `name=$(basename "$checkout")` and
`base="farhelm-test-worker/$name"`. When a caller passes `--checkout .` (e.g. from the checkout root), `name` is `.` and
`base` is `farhelm-test-worker/.`. On the worker, `scripts/test-worker/remote/with-lock.sh:22,25` resolves
`base=$(cd "$1" && pwd)` to `$HOME/farhelm-test-worker` and computes `worker=$(dirname "$base")` as `$HOME` instead of
`$HOME/farhelm-test-worker`. Consequently, `with-lock.sh:72,81` opens `$HOME/bootstrap.lock`, `$HOME/active.lock`,
`$HOME/holders/`, and checks `$HOME/bootstrapped` instead of `$HOME/farhelm-test-worker/...`: it bypasses mutual
exclusion with every other checkout on the worker, re-runs `bootstrap.sh` on every invocation (because
`scripts/test-worker/remote/bootstrap.sh:97` always writes the marker to `$HOME/farhelm-test-worker/bootstrapped`, never
`$HOME/bootstrapped`) even while other checkouts are running tests, and installs `src/`, `out/`, `stage/`, and `lock`
directly inside `$HOME/farhelm-test-worker/`. With `--checkout ..`, `base` resolves to `$HOME` itself, writing
`stage/`/`src/`/`out/` directly into `$HOME` and failing on `mkdir -p /home/holders`. Fix direction: canonicalize
`checkout` after option parsing in `test-worker.sh`
(`checkout=$(cd "$checkout" && pwd) || die "cannot find checkout $checkout"`) and reject empty/dot/slash basenames.

Reviewer metadata (g02-test-worker-isolation/f4): suggested bucket: other (cannot be in highest: maintainer test-worker
CLI argument handling bug; can corrupt worker lock isolation on the test worker host but does not affect shipped product
code or user data); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g02-test-worker-isolation/f4`.
