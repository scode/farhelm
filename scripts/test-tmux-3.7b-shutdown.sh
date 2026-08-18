#!/usr/bin/env bash
# Build the pinned tmux 3.7b binary when needed, then run the teardown
# regressions against that exact binary. Each cargo invocation gets a fresh
# test process so a surviving client or server cannot contaminate the next
# scenario.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
binary="$repo/.ci-tmux/tmux"
python_env="$repo/.ci-tmux-python"

if ! test -x "$binary"; then
  case "$(uname -s)/$(uname -m)" in
    Linux/x86_64) target=x86_64-unknown-linux-musl ;;
    Linux/aarch64 | Linux/arm64) target=aarch64-unknown-linux-musl ;;
    *)
      echo "the pinned tmux shutdown tests support Linux x86_64 and arm64" >&2
      exit 2
      ;;
  esac
  python3 -m venv "$python_env"
  "$python_env/bin/pip" install --require-hashes \
    -r "$repo/.github/release/ziglang-requirements.txt"
  zig=$("$python_env/bin/python" -c \
    'import pathlib, ziglang; print(pathlib.Path(ziglang.__file__).parent / "zig")')
  mkdir -p "$(dirname "$binary")"
  ZIG="$zig" "$repo/scripts/build-private-tmux.sh" "$target" "$binary"
fi

version=$($binary -V)
if test "$version" != "tmux 3.7b"; then
  echo "expected pinned tmux 3.7b at $binary, found: $version" >&2
  exit 1
fi

cd "$repo"
export PATH="$(dirname "$binary"):$PATH"

cargo test -p farhelm-supervisor \
  shutdown_acks_no_output_despite_unread_positional_replies \
  -- --show-output --test-threads=1
cargo test -p farhelm-supervisor \
  shutdown_survives_every_four_block_reply_boundary \
  -- --show-output --test-threads=1
cargo test -p farhelm-supervisor \
  an_abandoned_replay_candidate_is_reaped_after_no_output \
  -- --show-output --test-threads=1
cargo test -p farhelm --test e2e \
  terminal_conformance_holds_for_the_agent_and_for_a_tab \
  -- --show-output --test-threads=1
cargo test -p farhelm --test e2e \
  input_client_failure_safely_reaps_queued_output_before_reattach \
  -- --show-output --test-threads=1
cargo test -p farhelm --test e2e \
  connection_loss_safely_reaps_queued_output_before_reattach \
  -- --show-output --test-threads=1
cargo test -p farhelm --test e2e \
  takeover_reason_wins_over_a_gated_natural_detach \
  -- --show-output --test-threads=1
cargo test -p farhelm --test e2e \
  close_notifies_and_removes_a_tab_while_output_cleanup_is_pending \
  -- --show-output --test-threads=1
cargo test -p farhelm --test e2e \
  restart_restores_and_notifies_while_output_cleanup_is_pending \
  -- --show-output --test-threads=1
# The startup reap closes stale control clients that are carrying queued
# output — exactly the shape whose unsafe teardown aborts this tmux
# version — so it must be proven against the pinned binary, not only
# against whatever the distro ships.
cargo test -p farhelm --test e2e \
  a_killed_supervisor_leaves_no_orphaned_sink_client \
  -- --show-output --test-threads=1
