#!/usr/bin/env bash
# Build the pinned tmux binary when needed, then run the teardown
# regressions against that exact binary. Each recorded nextest invocation
# selects one scenario and retains its own report. The pinned nextest must
# already be on PATH; see docs/test-run-evidence.md for setup.
#
# Which version is "pinned" comes from .github/release/source-pins.env via
# scripts/build-pinned-tmux-ci.sh, the same pin the release payload build
# reads, so bumping the pin there bumps this suite with it. TODO.md's
# 2026-08-22 floor decision makes the product's version floor and this
# regression-tested pin one value; that floor lands in the supervisor in a
# later change of the same stack, and a script that hardcoded its own copy
# of the version would let the two drift apart silently once it has.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)

# Release callers retain the same tests while selecting release labels and
# the job's artifact root. Local calls use the recorder's private defaults.
record_args=(--kind development)
while [ "$#" -gt 0 ]; do
  case "$1" in
    --kind|--output-root)
      [ "$#" -ge 2 ] || { echo "missing value for $1" >&2; exit 2; }
      record_args+=("$1" "$2")
      shift 2
      ;;
    *) echo "usage: $0 [--kind development|repetition|release] [--output-root PATH]" >&2; exit 2 ;;
  esac
done

# The build (and the pinned-version assertion) lives in its own script so
# the full suite and the desktop smoke can share it; see its header.
binary_dir=$("$repo/scripts/build-pinned-tmux-ci.sh")

cd "$repo"
export PATH="$binary_dir:$PATH"

# Exact names plus nextest's no-tests failure make renames fail visibly,
# rather than turning an empty substring selection into a successful gate.
run_case() {
  local package=$1 test_name=$2
  shift 2
  python3 scripts/record-test-run.py --runner nextest "${record_args[@]}" \
    --selection "$package::$test_name" --concurrency '4 nextest slots; one selected test' --tmux required \
    -- cargo nextest run -p "$package" "$@" -E "test(=$test_name)"
}

run_case farhelm-supervisor tmux::stream::tests::shutdown_acks_no_output_despite_unread_positional_replies --lib
run_case farhelm-supervisor tmux::stream::tests::shutdown_survives_every_four_block_reply_boundary --lib
run_case farhelm-supervisor tmux::stream::tests::an_abandoned_replay_candidate_is_reaped_after_no_output --lib
run_case farhelm terminal_tabs::terminal_conformance_holds_for_the_agent_and_for_a_tab --test e2e
run_case farhelm terminal_tabs::input_client_failure_safely_reaps_queued_output_before_reattach --test e2e
run_case farhelm terminal_tabs::connection_loss_safely_reaps_queued_output_before_reattach --test e2e
run_case farhelm terminal_tabs::takeover_reason_wins_over_a_gated_natural_detach --test e2e
run_case farhelm terminal_tabs::close_notifies_and_removes_a_tab_while_output_cleanup_is_pending --test e2e
run_case farhelm terminal_tabs::restart_restores_and_notifies_while_output_cleanup_is_pending --test e2e
# The startup reap closes stale control clients that are carrying queued
# output — exactly the shape whose unsafe teardown aborted tmux 3.7b (the
# pin at the time; the suite follows the current pin) — so it must be
# proven against the pinned binary, not only against whatever the distro
# ships.
run_case farhelm terminal_tabs::a_killed_supervisor_leaves_no_orphaned_sink_client --test e2e
