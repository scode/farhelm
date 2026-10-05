# Restart with parent accepts unrestricted consent after its question is cancelled

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Restart with parent accepts unrestricted consent after its question is cancelled.

## Details

`F58 / COR-RESTART-PARENT-CANCEL` — **definite** — `crates/farhelm-ui/src/session_view.rs:2302` — Restart with parent
accepts unrestricted consent after its question is cancelled

Cancelling the confirmation for a restart with changed settings does not prevent the session view from accepting a stale
approval from that confirmation. The question appears when the edited launch would run in YOLO mode, meaning the agent
can run commands and change files without asking for approval, on a host that requires explicit confirmation before such
launches. Cancelling this question leaves the settings dialog open but clears `restart_yolo`, the parent session view's
record of the pending question. A confirmation callback already queued from the child dialog can still submit the edited
settings with `allow_yolo = true`. The parent's `on_submit` handler checks whether a restart is underway, whether the
session remains eligible, whether its saved launch changed, and whether the edit preserves the launch kind and agent
type. None of these checks establishes that the question being answered is still live.

The parent forwards that approval flag to the shared restart operation even when cancellation has removed the question.
The API client turns it into `confirm_yolo: true` in the request (`crates/farhelm-ui/src/api.rs:2126`). The helm accepts
this field as explicit approval and passes the restart to the supervisor without requiring the host's usual YOLO
confirmation (`crates/farhelm-helm/src/sessions.rs:2251` and `crates/farhelm-helm/src/yolo_guard.rs:51`). Thus, provided
the other restart requirements still hold, a stale answer can resume the conversation with an agent running without
approval prompts after the user cancelled. If the submitted dialog action also carried permission to stop a working
agent, the supervisor can stop that agent and its process tree before relaunching. The separate stop permission does not
restore the cancelled YOLO approval.

The permanent answer, “start, and don't ask again on this host,” has the same acceptance gap. The parent looks up the
host in the pending question, but if the question is absent it merely converts the requested preference change to
`None`. It still forwards `allow_yolo = true`, so the stale permanent answer becomes a one-off approved restart rather
than being refused. Require either approval-bearing answer to consume a live question bound to the current dialog
opening and the settings being approved before dispatch. The permanent answer must also have the host bound to that
question; a missing question or host must refuse the submission. A controlled regression should deliver a stale child
approval after the parent handles cancellation and verify that no restart is dispatched.

Suggested bucket: highest. This finding concerns the parent's acceptance of consent, an independently editable boundary
from the child's captured confirmation callbacks at `crates/farhelm-ui/src/restart_with.rs:608`, reported separately as
F22. The cancellation safeguards recorded in `TRIAGE_OUTCOMES.md:2229–2270` establish the live-confirmation requirement
for other flows, but do not cover this callback. Ordinary Restart also differs: it reuses the session's stored launch
rather than approving changed settings. No browser reproduction was performed. The queued-event premise is supported by
the existing confirmation tests and ledger; this finding was split from F22 during the location audit, not discovered in
a new review pass.

## Review context

Confidence as retained and filed: **definite**, source-established with the queued-event contract. Suggested queue
bucket: **highest**.

Origin: `ui_data p1`, parent callback at `session_view.rs:2302`. The child callback reports remain in
`yolo-restart-cancel.md`; different files were separated during location accounting.

Possible cover: TRIAGE_OUTCOMES2229–2270 describes live confirmation consumption in other flows; not exact coverage of
this independently editable parent callback. Ordinary Restart reuses consent and differs from Restart with new settings.

Caveats: No browser reproduction. Queued-event premise established by existing confirmation tests and ledger. Split from
F22 during location audit; same consequence at child and accepting parent kept separate, not a new review pass.
