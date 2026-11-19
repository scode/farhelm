#!/bin/bash
# Capture the docs website's screenshots on a bootstrapped boat sandbox, against binaries built on the development
# machine and unpacked at its own absolute path (`boat-tests.ts docs`, which ships them as `gui` does). A capture runs
# the whole staged fleet and the app under software rendering, CPU-bound work that a shared development machine under
# load starves until the capture's idle waits time out; a sandbox gives it cores of its own.
#
# Usage: run-docs.sh <checkout> <shipped|build> [<page>]. `shipped` runs the binaries the development machine sent;
# `build` has the capture build them here first, which is what lets a full capture write its publishable record
# (`capture.json`), since the capture then vouches for binaries built from the shipped source. With a page, only that
# page's shots (`--only`, for drafting); without, the full capture. Writes /home/user/out/docs/summary.txt, docs.log,
# and shots.tar.gz holding the captured PNGs and, when the run could write one, the full capture's record.
set -u
# As in run-gui.sh: drop the ambient capability boat's exec hands every command, so a run sees an ordinary login's
# permissions.
if [ -z "${BOAT_CAPS_DROPPED:-}" ] && ! grep -qE '^CapAmb:[[:space:]]*0+$' /proc/self/status; then
  export BOAT_CAPS_DROPPED=1
  exec setpriv --inh-caps=-all --ambient-caps=-all bash "$0" "$@"
fi
M=$1
MODE=$2
PAGE=${3:-}
case "$MODE" in
  shipped) build=(--no-build) ;;
  build) build=() ;;
  *) echo "run-docs.sh: mode must be shipped or build, not '$MODE'" >&2; exit 2 ;;
esac
OUT=/home/user/out/docs
SHOTS=website/public/docs-shots-local
rm -rf "$OUT"
mkdir -p "$OUT"
cd "$M" || exit 1
{
  echo "rsvg-convert: $(rsvg-convert --version 2>&1 | head -1)"
  for b in farhelm-desktop farhelm farhelm-fixtures gpui-agent; do
    echo "$b: $(test -x "target/debug/$b" && echo present || echo MISSING)"
  done
} >"$OUT/summary.txt"
rm -rf "$SHOTS"
# The capture runs as an account of its own, `fhshots`. The capture's leak check refuses any shot that draws the
# capturing account's name, and the sandbox's own account is `user`, a word the app prints in its own help (the Add
# host dialog's `user@host`), so as `user` that shot could never be taken and a full capture never be published. The
# account gets the tree, the pinned toolchain (installed first, as `user`, who owns rustup's directory), a home of its
# own for Cargo's downloads, and a lingering user manager like `user`'s, for the supervisor's transient scopes.
# `cargo --version` in the tree makes rustup install the toolchain rust-toolchain.toml pins, if it is missing.
cargo --version >/dev/null 2>&1 || { echo "installing the pinned toolchain failed" >>"$OUT/summary.txt"; exit 1; }
id -u fhshots >/dev/null 2>&1 || sudo -n useradd -m -s /bin/bash fhshots || exit 1
sudo -n loginctl enable-linger fhshots || exit 1
uid=$(id -u fhshots)
for _ in $(seq 50); do test -d "/run/user/$uid" && break; sleep 0.2; done
sudo -n chown -R fhshots:fhshots "$M" || exit 1
docs=(scripts/docs-screenshots.sh --local "${build[@]}")
[ -z "$PAGE" ] || docs+=(--only "$PAGE")
# shellcheck disable=SC2024 # the log is ours to write, as `user`, not the capture's
sudo -n -u fhshots env -i HOME=/home/fhshots USER=fhshots LOGNAME=fhshots LANG=C.UTF-8 \
  PATH=/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin CARGO_HOME=/home/fhshots/.cargo \
  RUSTUP_HOME=/usr/local/rustup XDG_RUNTIME_DIR="/run/user/$uid" \
  DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" "${docs[@]}" >"$OUT/docs.log" 2>&1
code=$?
# Hand the tree back, so the next run's reset can replace it.
sudo -n chown -R user:user "$M"
{
  echo "docs-screenshots: exit $code"
  grep -E '^(docs-shots|note|captured)' "$OUT/docs.log" | head -40
} >>"$OUT/summary.txt"
if [ -d "$SHOTS" ]; then
  tar -czf "$OUT/shots.tar.gz" -C "$SHOTS" . || code=1
fi
cat "$OUT/summary.txt"
exit "$code"
