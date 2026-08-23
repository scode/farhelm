#!/usr/bin/env bash
# Build the pinned tmux binary when needed, then run the teardown
# regressions against that exact binary. Each cargo invocation gets a fresh
# test process so a surviving client or server cannot contaminate the next
# scenario.
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

# The build (and the pinned-version assertion) lives in its own script so
# the full suite and the desktop smoke can share it; see its header.
binary_dir=$("$repo/scripts/build-pinned-tmux-ci.sh")

cd "$repo"
export PATH="$binary_dir:$PATH"

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
# output — exactly the shape whose unsafe teardown aborted tmux 3.7b (the
# pin at the time; the suite follows the current pin) — so it must be
# proven against the pinned binary, not only against whatever the distro
# ships.
cargo test -p farhelm --test e2e \
  a_killed_supervisor_leaves_no_orphaned_sink_client \
  -- --show-output --test-threads=1
