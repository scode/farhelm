# Pane capture can outlive its read deadline

Reviewed commit: 1ec60cc06baff3442bcdd73760a9ebdc8b08c49c

## TLDR

A terminal-status capture can keep the supervisor's periodic sampler waiting forever after stdout closes, so status and
conversation updates for every session on that supervisor stop advancing.

## Details

Found by pre-pr-review-swarm run `20261001-1343-1ec60cc-5337` as `F26 / COR-CAPTURE-DEADLINE`, tagged **definite**.

At `crates/farhelm-supervisor/src/tmux.rs:3457`, `run_bytes_tail` puts its deadline around stdout reads only. Once
stdout reaches EOF, it awaits the stderr reader and then the tmux child without a deadline; the timeout cleanup path
also joins stderr without a bound. A command can close stdout while remaining alive, or a descendant can keep stderr
open after the direct child exits.

The periodic sampler awaits captures sequentially, so one stalled capture can stop status sampling, screen recognition,
and related automatic cleanup for every session on that supervisor. Apply one deadline to stdout, stderr, and child
exit, with bounded retirement of reader tasks. Cleanup must target the short-lived capture client while preserving the
private tmux server.
