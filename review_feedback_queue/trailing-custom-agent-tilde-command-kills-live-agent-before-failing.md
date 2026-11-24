# Custom agent ~/ command expansion runs only in prepare_launch, killing live agents on Restart and allocating checkouts on Create before failing

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

When a registered custom agent uses a `~/...` command on a host whose supervisor cannot expand `~` (missing, empty,
relative, or non-UTF-8 `HOME`), Restart and Restart with stop and kill the live agent and its entire process tree before
discovering the command cannot be expanded, and a managed-checkout Create allocates the checkout directory and leaves a
retained error session before failing.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:14621-14633`.

`Supervisor::prepare_launch` (`crates/farhelm-supervisor/src/service/core.rs:14621-14633`) is the only place that checks
and expands a custom agent's `~`-prefixed executable via
`expand_tilde_path("custom agent command", program, self.user_home.as_deref())`. Neither
`Supervisor::restart_session_named` (`core.rs:12547-12566`) nor `Supervisor::validate_create` (`core.rs:9156-9178`)
pre-checks this expansion. `core.rs:6278-6284` explicitly permits starting a supervisor with `user_home: None` (when
`HOME` is unset or empty) or a relative/non-UTF-8 `HOME`, expecting individual `~` requests to be refused cleanly.

1. On Restart / Restart with (established shipped route): `SPEC.md:2202-2204` states: "A working agent is stopped first
   with the user's confirmation on that action; a live agent in any other status is stopped first without it, as for
   Restart. The edited launch is validated exactly as a create validates it before anything is stopped, so an invalid
   edit leaves the agent running and the dialog open with the reason." Accordingly, `restart_session_named` pre-checks
   command placeholders (`core.rs:12547-12558`) and `cwd` (`core.rs:12562-12574`) before calling `stop_live_agent`
   (`core.rs:12632`). For a live custom-agent session on a host whose supervisor lacks a usable `HOME` — reached in
   shipped code either by editing a custom agent's command from an absolute/PATH name to `~/bin/agent` in Settings and
   running **Restart with**, or by restarting the supervisor without `HOME` while a `~/bin/agent` session survives in
   its terminal host and then running **Restart** — `stop_live_agent` kills the live agent and its entire process tree
   (without asking for confirmation when the agent is idle or waiting), and only then does `relaunch` ->
   `relaunch_into_host` (`host_restart.rs:239-273`) -> `prepare_launch` (`core.rs:14630`) fail deterministically with
   `HostSpawnFailure::Spec` / `RelaunchFailure::Definitive`. The live process tree is lost.
2. On managed-checkout Create: `validate_create` expands `~` for `cwd` (`core.rs:9137`) and `checkout_root`
   (`core.rs:9396`) before session insertion or directory allocation, but does not validate the custom agent command's
   `~` expansion. `launch_reserved` therefore inserts the session row, runs `mkdir` to allocate the managed checkout
   directory (`core.rs:11309`), writes preparation state (`core.rs:11356`), and only then fails in `prepare_launch`
   (`core.rs:14630`), hitting `FailedStart::Spec` with `allocated_checkout.is_some()` (`core.rs:11637-11649`) and
   leaving an allocated checkout directory and retained error session on disk instead of refusing before allocation.

How to verify: Inspect `core.rs:6278-6284` (`user_home` initialization), `core.rs:9156-9178` (`validate_create`),
`core.rs:11307-11662` (`launch_reserved`), `core.rs:12547-12656` (`restart_session_named`), and `core.rs:14621-14633`
(`prepare_launch`).

Fix: Extract a shared helper (e.g. `validate_custom_agent_command_expansion(argv, snapshot, self.user_home.as_deref())`)
and call it in `validate_create`, `validate_retry`, and `restart_session_named` before `stop_live_agent`, keeping
`prepare_launch`'s expansion as the execution-time boundary.

Reviewer metadata (g42b_custom_agents_sup_service/f2): suggested bucket: highest (borderline `high`; surfacing the
boundary explicitly per `review_feedback_queue/AGENTS.md`, "Priority buckets" and `recording.md`). Why `highest`: the
shipped route is fully established (not speculative), and `review_feedback_queue/AGENTS.md` places bugs that lose
user-owned processes in `highest` regardless of trigger rarity — here, an idle or waiting agent and its entire process
tree are killed unasked before a deterministic preflight error that `SPEC.md:2203-2204` requires refusing with the agent
still running. Why triage may judge it `high` instead: the user initiated Restart/Restart with expecting the process to
be replaced, and the durable conversation ID on disk is preserved so a later Restart after fixing `HOME` or the command
can still resume it (only the live process and any unsaved in-process/child-process state are lost). Per `recording.md`,
uncertainty at the `highest` boundary is treated as `highest` until triage decides.; confidence: confirmed (verified
directly from source at `1b3fe49af86979aebc2719965682a17f8c6366ac`).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g42b_custom_agents_sup_service/f2`.
