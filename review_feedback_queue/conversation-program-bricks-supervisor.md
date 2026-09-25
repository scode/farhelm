# A {conversation} program stores a row that stops the supervisor starting

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A mistyped command or profile like `{conversation} …` with kind Claude creates an errored session, and later that host's
supervisor refuses to start.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F1 / COR-CONVERSATION-PROGRAM`, tagged **definite**. Anchors and title: `service/core.rs:7115-7125`,
`service/core.rs:7196-7205`, `agent_kind/mod.rs:2903-2921`, `store.rs:2650-2662`, `service/core.rs:5355` — A create
whose program is literally `{conversation}` stores a resume template the row loader later refuses, so the supervisor
cannot start

The supervisor is the per-host daemon that runs agents in tmux and keeps a SQLite row for each session. When it creates
a session it also saves a _resume template_: the argv that a later Restart runs to reopen the agent's conversation. In
that argv, the placeholder `{conversation}` stands for the captured conversation id. The caller can supply a template.
If the caller does not, and the session has an integrated agent kind (Claude, Codex, Goose, Pi, OMP, Grok),
`IntegrationSnapshot::resolve` builds one by copying the original argv and adding the kind's resume flags. For Claude
that is `original_argv + ["--resume", "{conversation}"]` (`agent_kind/mod.rs:1196`). The shared checker
`ensure_resume_template` refuses any template whose first element, the program, is `{conversation}`. At create time,
though, `validate_create` runs that check only on a caller-supplied template (`core.rs:7121`), never on the derived one.
The invocation checks that do run, `ensure_executable_argv` and `ensure_no_cwd_program`, refuse only `{cwd}` as the
program. `farhelm_proto::validate_profile_fields` has the same gap.

So a raw create whose command is `{conversation} …` with a kind override of Claude, or a legacy profile with
`agent_kind: claude` and that invocation, is accepted. It stores the template
`["{conversation}", …, "--resume", "{conversation}"]`. The launch itself fails because no program named `{conversation}`
exists, so the session shows as errored. The real damage comes later. When rows are loaded, `decode_session_row` does
run `ensure_resume_template` (`store.rs:2657`) and bails on this row. `load_all` collects rows with `?`, so one bad row
fails the whole load. That failure passes through `reload_sessions` (`core.rs:5355`), and the supervisor constructor
fails. The next supervisor restart, upgrade or reboot leaves the host with no working supervisor. Delete, the command
that could remove the row, needs a running supervisor, so the only way back is editing SQLite by hand.

The fix is to run `ensure_resume_template` on the resolved `snapshot.resume_template` in `validate_create`, whether the
template was supplied or derived. That makes create and load agree on which templates are valid. Optionally,
`validate_profile_fields` could also refuse `{conversation}` as the program, so the profile editor catches the mistake
before any create.
