# Suspend is silently dropped if a launch on the same host starts during its activity check

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If you click Suspend (or Stop and suspend) and a launch on the same host starts while Farhelm is still checking that
host's session activity, Suspend quietly does nothing instead of showing why it did not run.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_list/environment.rs:473`.

Note first that the `launch_refusal.or_else(|| help(host, verb))?` fallback in
`crates/gui/farhelm-gui/src/hosts/lifecycle.rs:104` and `hosts/menu.rs:345` is safe because `hosts/offers.rs:380-381`
(`lifecycle_of`) requires `HostKind::provider().is_some()`, where `lifecycle::help` is always `Some`. However, in both
`crates/gui/farhelm-gui/src/session_list/environment.rs:464-482` (`begin_shown`) and
`crates/gui/farhelm-gui/src/hosts/lifecycle.rs:137-154,158-163,178-181` (`suspend`/`offering_suspend`/`ask`), clicking
Suspend (or row-menu Stop and suspend) starts an asynchronous `GET /api/sessions?host=<id>` activity read
(`read_activity`). If a launch targeting that host starts before that read answers (for example, an already-preparing
quick launch finishing its catalog read, or a launch submitted while the read is out), `environment.rs:473-475` returns
silently without opening the confirmation or recording a refusal, and `hosts/lifecycle.rs:144,179` likewise returns
silently via `offering_suspend` returning `None` (and `environment.rs:665-667` / `hosts/lifecycle.rs:258-266` also drop
silently if reached via a race). Contrast `SPEC.md` "Errors and diagnostics" L4801 ("Every failed operation surfaces a
concrete, actionable error in the client"), `crates/gui/farhelm-gui/src/session_list/menu.rs:794-816`
(`refuse_for_host_change`, which writes the reason to `operations.note`), and the reference Dioxus app
(`24bc0580:crates/farhelm-ui/src/hosts.rs:2324-2331`, `sandbox_sessions.rs:369-376`), which surfaced
`LAUNCH_ON_THIS_HOST` on the host/session row when a launch superseded an in-flight activity read. (When the Suspend or
Stop confirmation is already open, `Prompt::disabled_while` keeps the dialog open with the disabled button and tooltip,
so no dialog closes silently.) Fix direction: when `host_lifecycle_refusal` blocks completion after `read_activity`
returns (or in `send`), record the refusal reason via `operations.note(...)` on the session/host key (when nothing else
is in flight on that key), matching `refuse_for_host_change`.

Reviewer metadata (g09-wake-host-suspend-guard/l3): suggested bucket: other (could NOT be in highest: no security,
credential, or data-loss impact; the host stays running and the user's launch proceeds, while the disabled Suspend
button's tooltip explains the in-flight launch); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g09-wake-host-suspend-guard/l3`.
