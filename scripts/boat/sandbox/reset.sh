#!/bin/bash
# Return a reused boat sandbox to a clean state between test runs: stop what an earlier run left (Farhelm processes and
# their transient user scopes, a GUI run's app and compositor, containers), and remove its run directories and outputs.
# A sandbox is kept across runs because boat charges per start, so nothing from one run may leak into the next.
#
# Usage: reset.sh [keep-stage]. `keep-stage` keeps the uploaded packs and the unpacked tree, so a rerun of the same
# archive (`boat-tests.ts rust --reuse-stage`) sends nothing again.
set -u
XDG_RUNTIME_DIR=/run/user/$(id -u)
export XDG_RUNTIME_DIR
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
# Transient scopes and services the supervisors under test created.
for u in $(systemctl --user list-units --all --plain --no-legend 'farhelm-*' 2>/dev/null | awk '{print $1}'); do
  systemctl --user stop "$u" >/dev/null 2>&1
done
systemctl --user reset-failed >/dev/null 2>&1
pkill -u "$(id -u)" -f 'target/debug/' 2>/dev/null
pkill -u "$(id -u)" -f 'cargo-nextest' 2>/dev/null
pkill -u "$(id -u)" -f 'gui-e2e-smoke.py|gui-lifecycle.py|gui-agent.py' 2>/dev/null
pkill -u "$(id -u)" -x mutter 2>/dev/null
podman ps -aq 2>/dev/null | xargs -r podman rm -f >/dev/null 2>&1
sleep 1
rm -rf /tmp/farhelm-tmp /home/user/out
[ "${1:-}" = keep-stage ] || rm -rf /home/user/stage
mkdir -p /home/user/stage /home/user/out
echo "reset ok"
