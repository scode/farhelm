# Restart-with skips the create-time argv and template checks, so a bad bundle can stop the supervisor from starting

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

When a session is restarted with new launch settings ("restart with"), the host's supervisor saves the new command line
and resume command without the checks it applies when a session is created. If those settings have a shape that create
refuses, the supervisor saves them anyway. On its next start it cannot load its session database, so every session on
that host is unreachable until someone repairs the database by hand. The helm shipped today always builds these settings
from a fixed per-harness program, so this doesn't happen in normal use. It is a gap in a boundary the code enforces
everywhere else, and it contradicts SPEC_impl.md.

## Details

Source: whole-codebase review, 2026-09-30, slice sup-core.

Reviewer's confidence: confirmed (traced end to end; the shipped helm never sends a triggering bundle, see below).

Reviewer's bucket suggestion: other.

Possible cover for triage to check: none. SPEC_impl.md "Restart-with backend wire and persistence" describes the
opposite of the code: "then resolves and fills the supplied template using the same executable and integration checks as
create".

The restart-with branch of `Supervisor::restart_session` (`crates/farhelm-supervisor/src/service/core.rs`, about lines
9982-10040) validates the supplied bundle only with:

- `shell_words::split` plus `agent_kind::ensure_executable_argv` on the invocation, which checks for a non-empty argv0
  and no NUL bytes.
- `IntegrationSnapshot::resolve(&new_argv, Some(snapshot.kind), resume_template)` (`agent_kind/mod.rs:2139-2172`). This
  only checks whether the placeholder is present or absent for the kind. It never calls `ensure_resume_template`.

The create path (`validate_create`, core.rs:7068-7078 and 7156-7165) additionally applies:

- `ensure_no_cwd_program` to the invocation.
- `ensure_resume_template` to both an override template and a derived one. Its comment gives the reason: "Loading fails
  as a whole on one bad row, so accepting it here would leave the supervisor unable to start after its next restart."
- The handler-level `RESUME_TEMPLATE_ELEMENT_CAP` and byte caps (`handlers.rs:840-870`). `handle_restart_session`
  (`handlers.rs` around line 2324) applies none of these.

After a successful spawn, `relaunch` writes the bundle unchanged through `SessionStore::update_restart_with_bundle`
(core.rs about 10382-10420; store.rs:4720-4754, which does no validation of its own).

- `decode_session_row` (store.rs:2690-2715) refuses a row whose template fails `ensure_resume_template` (for example
  `{conversation}` or `{cwd}` as argv0), or whose invocation has `{cwd}` as argv0.
- `load_all` (store.rs:5544-5567) uses `?` over every row, so one refused row fails the whole load.
- `reload_sessions` propagates that failure (`store.load_all().await?`, core.rs about 5318), so the supervisor fails to
  construct or serve on every start.
- `store.session(id)` also fails for that row, so restart, report admission and snapshot reads for the session error out
  immediately.

Concrete trigger: send `RestartSession` with `invocation`, `launch` (same harness as stored) and
`resume_template = ["{conversation}", "x"]`, or a valid template with an invocation whose argv0 is `{cwd}`, to a session
whose offer is Resume.

- `resolve` accepts it because the placeholder is present.
- The spawn succeeds, since tmux starts the launch shim and the exec failure only shows up later.
- The relaunch publishes, the bundle is written, and the next supervisor start fails in `load_all`.

Reachability: the helm builds this bundle only through `launches::compile` (`crates/farhelm-helm/src/launches.rs:340`).
Its argv0 is always `program(harness)` and its template is derived from that, so no shipped client can send a triggering
bundle. Any other client of the supervisor socket could, and so could a future change to `compile`. The supervisor
trusts the helm, yet it still enforces these rules at create, because the consequence is a supervisor that cannot start.

Fix: in the restart-with branch, apply the same checks as `validate_create`, and refuse with `InvalidRequest` before any
destructive work:

- `ensure_no_cwd_program` on `new_argv`;
- `ensure_resume_template` on the resolved `integration.resume_template`;
- the element and byte caps, in the handler as the create handler does.

To verify, add a restart-with test with template `["{conversation}"]` and assert a refusal and an unchanged row. Today
the restart succeeds, and a fresh `Supervisor` on the same state directory then fails to construct.
