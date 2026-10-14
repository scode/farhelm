# Quick launch can use another session's destination

Reviewed commit: 4a90caa2f6b3b2e3e082211602d4b9dc82d42287

## TLDR

Quick launch can run a saved command on a different host or in a different project if you select another session while
its template is loading.

## Details

Suggested bucket: `highest`. Confidence: Confirmed static mechanism; no browser reproduction. Source: dedicated
`gpt-6.1-sol` high adversarial review of this commit. Proposed drop: none.

The shortcut starts preference/template reads without capturing the inherited destination
(`crates/farhelm-ui/src/list/view.rs:3339–3356`). Session selection remains admitted by `guarded_open` (`2794–2808`),
which neither checks `quick_launch_pending` nor revokes `opening_generation`. After the reads, `3375–3381` calls the
live `open_new` callback, whose current closure reads the latest selected host and destination (`3301–3320`). Dioxus
0.7.10's `use_callback` updates that closure every render.

Mark a runnable template that omits host and destination. Select session A, invoke quick launch, hold `/api/templates`,
select session B on another host or in another project, and release the response. The automatic launch inherits B's
destination. A command that changes files can affect the wrong project. The window lasts as long as the reads, so it
need not be sub-second.

Capture the inherited destination on shortcut admission, or cancel the pending shortcut on selection changes. Preserve
current host-installation verification when doing so.

Possible cover: SPEC.md:641–655 describes New's defaults without an explicit capture instant. SPEC_impl.md:865–868
promises isolation from background interaction, but the modal starts after these initial reads. Neither explicitly
accepts destination substitution.

Current-main status: persists by inspection at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; no fixing commit found. The
reviewer read the full commit diff and relevant traces; this is not an exhaustive review of unchanged surrounding code.
