#!/usr/bin/env bash
# Shared Mac shipment for both media commands. Preflight and source rsync take the Mac-wide lock. The remote capture
# gives the same lock to gui-agent, which keeps it from build through teardown. Fetching the output needs no lock.
# Callers define mac_capture_script and fetch_from_mac. The latter receives the remote result path and capture
# status. It uses remote or ssh_cmd from capture_on_mac's scope.

# The preflight: refuse a Mac where the window could not open or the build would not fit, failing closed when a figure
# cannot be read. Argument: the lock, under the Mac's home.
mac_preflight_script() {
  cat <<'EOF'
preflight() {
  console=$(/usr/bin/stat -f %Su /dev/console) || return 1
  me=$(/usr/bin/id -un) || return 1
  free=$(/bin/df -g "$HOME" | /usr/bin/awk 'NR==2 {print $4}')
  mem=$(/usr/bin/vm_stat | /usr/bin/awk '/page size of/ {ps=$8} /Pages free/ {f=$3} /Pages inactive/ {i=$3} END {printf "%d", (f+i)*ps/1073741824}')
  for figure in "$free" "$mem"; do
    case "$figure" in
      "" | *[!0-9]*) echo "could not read the Mac's free disk ('$free') or memory ('$mem')" >&2; return 1 ;;
    esac
  done
  echo "the Mac: console user $console, $free GB of disk and $mem GB of memory free"
  if [ "$console" != "$me" ]; then
    echo "the Mac's console belongs to $console, not $me, so no window of this user can open there" >&2
    return 1
  fi
  if [ "$free" -lt 15 ]; then
    echo "refusing: the Mac has under 15 GB of free disk" >&2
    return 1
  fi
  if [ "$mem" -lt 4 ]; then
    echo "refusing: the Mac has under 4 GB of free memory" >&2
    return 1
  fi
}
export -f preflight
cd "$HOME" || exit 1
/usr/bin/lockf -k "$1" /bin/bash -c preflight
EOF
}

# Whether a value is safe to put on a remote command line unquoted: a relative path of letters, digits, `.`, `_`, `-`
# and `/`, with no `..` component.
plain_path() {
  case "$1" in
    "" | /* | -* | *[!A-Za-z0-9._/-]* | .. | ../* | */.. | */../*) return 1 ;;
  esac
  return 0
}

capture_on_mac() {
  local repo="$1" mac="$2" build="$3" name="$4"
  shift 4
  local ssh_cmd base lockfile id src out tmp status=0
  case "$mac" in
    "" | -* | *[[:space:]]*) echo "--mac needs an ssh destination such as user@host, not '$mac'" >&2; exit 2 ;;
  esac
  read -r -a ssh_cmd <<<"${FARHELM_MEDIA_SSH:-ssh}"
  base="${FARHELM_MEDIA_MAC_DIR:-farhelm-media}"
  lockfile="${FARHELM_MEDIA_MAC_LOCK:-.farhelm-gpui-mac.lock}"
  plain_path "$base" || { echo "FARHELM_MEDIA_MAC_DIR must be a plain path under the Mac's home" >&2; exit 2; }
  plain_path "$lockfile" || { echo "FARHELM_MEDIA_MAC_LOCK must be a plain path under the Mac's home" >&2; exit 2; }
  command -v rsync >/dev/null || { echo "shipping the checkout needs rsync" >&2; exit 1; }
  # One source directory per checkout on the Mac, named from this checkout's path, so two checkouts never ship over
  # each other and one checkout's builds there stay incremental.
  id="$(printf '%s' "$repo" | cksum | cut -d' ' -f1)" || exit 1
  src="$base/src-$id"
  tmp="$(mktemp -d)" || exit 1
  # The Mac can keep evidence from a failure without the next run picking it up as a fresh capture.
  out="$base/${tmp##*/}-$name"
  # shellcheck disable=SC2064 # expand now: the path is this call's own
  trap "rm -rf '$tmp'" EXIT
  remote() { "${ssh_cmd[@]}" "$mac" "$@"; }
  # The test Mac's SSH server once returned 0 after a remote exit 3. Require Bash's own status line and fail when
  # it never arrives. rsync checks its protocol independently of this marker.
  checked_script() {
    local status
    { cat; printf '\necho "remote-status:$?"\n'; } | remote bash -s -- "$@" | tee "$tmp/remote.log"
    status="$(sed -n 's/^remote-status:\([0-9][0-9]*\)$/\1/p' "$tmp/remote.log" | tail -n 1)"
    return "${status:-125}"
  }

  mac_preflight_script | checked_script "$lockfile" || { echo "the Mac refused the capture (above)" >&2; exit 1; }

  # What git would not ignore, by every .gitignore in the checkout, which the sending rsync reads the way git does; not
  # git itself, so a jj workspace without a git working tree ships the same files. The Mac's rsync (openrsync) does not
  # apply those rules to what `--delete` may remove there, so the shipped tree's own build directory is named too, or
  # every run would delete it and build from nothing.
  echo "shipping the checkout to $mac:$src"
  rsync -a --delete --filter=':- .gitignore' --exclude=/.git --exclude=/.jj --exclude=/target \
    -e "${ssh_cmd[*]}" --rsync-path="/usr/bin/lockf -k $lockfile rsync" "$repo/" "$mac:$src/" || {
    echo "shipping the checkout to the Mac failed" >&2
    exit 1
  }

  mac_capture_script | checked_script "$src" "$lockfile" "$out" "$out.log" "$build" "$@" || status=$?
  if [ "$status" -eq 125 ]; then
    # Each run has fresh remote names, so name what stays behind for whoever cleans up.
    echo "the Mac did not report a capture status; its output and log, if any, stay at $mac:$out and $mac:$out.log" >&2
    return 1
  fi
  if [ "$status" -ne 0 ]; then
    echo "the Mac capture failed (above); its run directory, if named, and its log at $mac:$out.log are kept there" >&2
    # The docs preview can still use individual shots that passed their checks.
    fetch_from_mac "$out" "$status"
    return 1
  fi
  fetch_from_mac "$out" 0 || return 1
  # Cleanup has its own outcome once the checked result has reached the checkout.
  remote "/bin/rm -f $out.log" || echo "could not remove the Mac capture log" >&2
}
