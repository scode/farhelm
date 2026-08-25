#!/usr/bin/env bash
# Prove one Linux release payload is a fully static ELF built for the
# architecture its name claims.
#
# ## Why this is its own script
#
# Two places need exactly this verdict and must not drift: the tmux producer
# (`scripts/build-tmux-assets.sh`, checking what it just built) and the release
# signing gate (`.github/workflows/sign-sums.yml`, checking what was actually
# published). A payload that fails here reaches a host we do not control, where
# a missing loader or a wrong machine is a first-exec failure with nothing
# nearby to explain it.
#
# ## The failure mode this is shaped around
#
# `if readelf -d "$binary" | grep -q NEEDED; then …` looks like a check and is
# not one. Bash exempts an `if` condition from `errexit`, so a readelf that
# CANNOT READ THE FILE takes the same branch as a readelf that read it and
# found nothing — and the caller then announces the payload is static. Every
# inspection below therefore captures its output in an assignment that fails
# the script when the tool fails, and only then searches the captured text.
#
# Usage: check-static-elf.sh <binary> <target-triple>
#        check-static-elf.sh --self-test
#
# Requires: file(1), readelf(1). Both read the target's headers, so this works
# from any host architecture — the aarch64 payload is checked on x86_64.

set -euo pipefail

# The ELF machine string `readelf -h` prints for each target we publish. Kept
# here rather than at the call sites so "which triple means which machine" has
# one answer in the repository.
expected_machine_for() {
  case "$1" in
    x86_64-unknown-linux-musl) echo 'Advanced Micro Devices X86-64' ;;
    aarch64-unknown-linux-musl) echo 'AArch64' ;;
    *)
      echo "no expected ELF machine known for target $1" >&2
      return 1
      ;;
  esac
}

# Exit 0 only when every inspection SUCCEEDED and said what it must.
check_static_elf() {
  local binary=$1 target=$2 machine described header dynamic program_headers
  machine=$(expected_machine_for "$target") || return 1

  test -f "$binary" || {
    echo "$binary: not a file" >&2
    return 1
  }

  described=$(file -b "$binary") || {
    echo "$binary: file(1) could not describe it" >&2
    return 1
  }
  case "$described" in
    *"statically linked"* | *"static-pie linked"*) ;;
    *)
      echo "$binary is not statically linked: $described" >&2
      return 1
      ;;
  esac

  header=$(readelf -h "$binary") || {
    echo "$binary: readelf could not read the ELF header" >&2
    return 1
  }
  if ! printf '%s\n' "$header" | grep -Eq "Machine:[[:space:]]+$machine"; then
    echo "$binary is not built for $machine (target $target):" >&2
    printf '%s\n' "$header" | grep -E '^[[:space:]]*Machine:' >&2
    return 1
  fi

  dynamic=$(readelf -d "$binary") || {
    echo "$binary: readelf could not read the dynamic section" >&2
    return 1
  }
  if printf '%s\n' "$dynamic" | grep -q NEEDED; then
    echo "$binary has dynamic library dependencies:" >&2
    printf '%s\n' "$dynamic" | grep NEEDED >&2
    return 1
  fi

  program_headers=$(readelf -l "$binary") || {
    echo "$binary: readelf could not read the program headers" >&2
    return 1
  }
  if printf '%s\n' "$program_headers" | grep -q INTERP; then
    echo "$binary carries a dynamic interpreter:" >&2
    printf '%s\n' "$program_headers" | grep -A1 INTERP >&2
    return 1
  fi

  echo "ok: $binary is a static $machine ELF"
}

# --------------------------------------------------------------------------
# Self-test
#
# Driven entirely by SHIMS for `file` and `readelf` on PATH, so it needs no
# real static binary and runs in milliseconds. The point is not to test
# binutils: it is to prove that each way an inspection can go wrong — including
# the tool FAILING, which is the bug this script exists to prevent — ends in a
# refusal rather than in "ok".
# --------------------------------------------------------------------------

write_shims() {
  local dir=$1
  mkdir -p "$dir"

  cat >"$dir/file" <<'SHIM'
#!/usr/bin/env bash
case "$SHIM_MODE" in
  file-fails) exit 1 ;;
  dynamic) echo "ELF 64-bit LSB pie executable, x86-64, dynamically linked, stripped" ;;
  *) echo "ELF 64-bit LSB pie executable, x86-64, static-pie linked, stripped" ;;
esac
SHIM

  # One shim for all three readelf modes; `$1` is the flag the caller passed.
  cat >"$dir/readelf" <<'SHIM'
#!/usr/bin/env bash
case "$1" in
  -h)
    [ "$SHIM_MODE" = header-fails ] && exit 1
    if [ "$SHIM_MODE" = wrong-machine ]; then
      echo "  Machine:                           AArch64"
    else
      echo "  Machine:                           Advanced Micro Devices X86-64"
    fi
    ;;
  -d)
    [ "$SHIM_MODE" = dynamic-fails ] && exit 1
    if [ "$SHIM_MODE" = needed ]; then
      echo " 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]"
    else
      echo "There is no dynamic section in this file."
    fi
    ;;
  -l)
    [ "$SHIM_MODE" = headers-fail ] && exit 1
    if [ "$SHIM_MODE" = interp ]; then
      echo "  INTERP         0x0000000000000318 0x0000000000000318"
      echo "      [Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]"
    else
      echo "  LOAD           0x0000000000000000 0x0000000000000000"
    fi
    ;;
esac
exit 0
SHIM

  chmod +x "$dir/file" "$dir/readelf"
}

self_test() {
  local work shims failures=0
  work=$(mktemp -d)
  # shellcheck disable=SC2064 # expand $work now: the trap must survive the local
  trap "rm -rf '$work'" RETURN
  shims="$work/bin"
  write_shims "$shims"
  : >"$work/payload"

  run_case() {
    local mode=$1 want=$2 out
    if out=$(SHIM_MODE=$mode PATH="$shims:$PATH" check_static_elf "$work/payload" \
      x86_64-unknown-linux-musl 2>&1); then
      if [ "$want" = accept ]; then
        echo "  ok: $mode accepted"
      else
        echo "self-test: $mode was ACCEPTED; it must be refused" >&2
        failures=$((failures + 1))
      fi
    else
      if [ "$want" = reject ]; then
        echo "  ok: $mode refused — $(printf '%s' "$out" | head -1)"
      else
        echo "self-test: a healthy payload was refused — $out" >&2
        failures=$((failures + 1))
      fi
    fi
  }

  run_case healthy accept
  # The four "the tool did not answer" cases. Every one of these was silently
  # accepted by the `if cmd | grep -q` shape this script replaced.
  run_case file-fails reject
  run_case header-fails reject
  run_case dynamic-fails reject
  run_case headers-fail reject
  # The three "the tool answered, and the answer is wrong" cases.
  run_case dynamic reject
  run_case wrong-machine reject
  run_case needed reject
  run_case interp reject

  # An unknown triple has no expected machine and must not fall through.
  if PATH="$shims:$PATH" check_static_elf "$work/payload" riscv64-unknown-linux-musl >/dev/null 2>&1; then
    echo "self-test: an unknown target triple was ACCEPTED" >&2
    failures=$((failures + 1))
  else
    echo "  ok: an unknown target triple is refused"
  fi

  if [ "$failures" -ne 0 ]; then
    echo "self-test FAILED: $failures case(s)" >&2
    return 1
  fi
  echo "== self-test ok: every failed or wrong inspection is refused"
}

if [ "${1:-}" = "--self-test" ]; then
  self_test
  exit 0
fi

binary=${1:?usage: check-static-elf.sh <binary> <target-triple>}
target=${2:?usage: check-static-elf.sh <binary> <target-triple>}
check_static_elf "$binary" "$target"
