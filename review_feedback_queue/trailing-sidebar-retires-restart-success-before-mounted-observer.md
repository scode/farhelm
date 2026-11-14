# Sidebar retirement can consume a restart result before the open view observes it

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: a successful Restart can leave its already-open session showing the interrupted card, or trying
the replaced terminal kind, until the next successful detail read. The sidebar can consume the success before the view
uses it. Synthesis revision: preserve the original consequence but distinguish a generation increment from mounting an
actual terminal. g088/l1 correctly establishes that restart_revision survives retirement: session_view.rs:549-564
increments mount_generation even when restart is None. Its isolated observer test at :3956-4013 checks this scalar and
does not mount SessionView or assert relaunched. That does not settle surface admission: :1722-1730 emits an empty
terminal set while keep_terminal_surface is false; :3711-3732 returns false for nonstale Interrupted with
relaunched=false regardless of mount_generation. The success payload is the only completion path setting relaunched=true
(:559-561), and at this revision also carries the new terminal kind. Thus the consequence is missing terminal
admission/override, not a lost increment. Source-established sequence at the reviewed earlier revision: the mounted
current signal still describes the interrupted or replaced run; a successful POST publishes restart and
restart_revision; a listing started AFTER that publication (generation above restart_listing_floor) commits a positive
new-run status before the deferred mounted observer, clearing restart at session_ops.rs:219-233. The view still has its
own earlier current while its independent detail is pending. Sidebar listing commits do not copy their status into that
cu

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g036/l1`.

### g036/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: b3d6b3eff6b7508e0b1fb6ec5b5cde31d27ebabe. TLDR: a successful Restart can leave its already-open session
showing the interrupted card, or trying the replaced terminal kind, until the next successful detail read. The sidebar
can consume the success before the view uses it. Synthesis revision: preserve the original consequence but distinguish a
generation increment from mounting an actual terminal. g088/l1 correctly establishes that restart_revision survives
retirement: session_view.rs:549-564 increments mount_generation even when restart is None. Its isolated observer test at
:3956-4013 checks this scalar and does not mount SessionView or assert relaunched. That does not settle surface
admission: :1722-1730 emits an empty terminal set while keep_terminal_surface is false; :3711-3732 returns false for
nonstale Interrupted with relaunched=false regardless of mount_generation. The success payload is the only completion
path setting relaunched=true (:559-561), and at this revision also carries the new terminal kind. Thus the consequence
is missing terminal admission/override, not a lost increment.

Source-established sequence at the reviewed earlier revision: the mounted current signal still describes the interrupted
or replaced run; a successful POST publishes restart and restart_revision; a listing started AFTER that publication
(generation above restart_listing_floor) commits a positive new-run status before the deferred mounted observer,
clearing restart at session_ops.rs:219-233. The view still has its own earlier current while its independent detail is
pending. Sidebar listing commits do not copy their status into that current; the selected-session bridge at
session_view.rs:1252-1269 updates title only and requests detail. The observer then increments generation but cannot set
relaunched or the kind override. An admitted, post-boundary connected detail repairs the view when it returns. This
finding does not require a newly fetched connected detail to return cached Interrupted; it is the already-mounted old
current before that detail returns. The original old-kind attachment consequence remains specific to this earlier
terminal-kind protocol.

Pinned ordering checked: Cargo.lock uses Dioxus core/hooks/desktop 0.7.10; installed primary dependency
use_effect.rs:15-47 queues a callback that reads the latest state when it executes; core virtual_dom.rs:507-537 drains
ready tasks before effects, :599-624 renders/tasks before finish_render, and scheduler.rs:61-78 ranks effects last.
Desktop webview.rs:537-590 waits for browser mutation delivery before continuing virtual-DOM work. A post-completion
listing task can therefore become ready during that delivery interval and retire success before the observer; there is
no observer-first guarantee or captured completion payload. All evidence is source/contract analysis, not observed
execution.

Cross-group boundary: g037/f3 and g038/l1 concern an OLD STALE listing accepted after
f560058da19e02716e6f943883a9508b447aff4e removes the generation floor. Their age-of-evidence defect can be fixed by
restoring that floor; this earlier finding already has the floor and uses a newer eligible listing, so that fix does not
settle it. They share the wider lifetime contract (new-view seed retirement must not erase unconsumed mounted
completion), and a durable per-consumer completion design could fix both. Retain the two triggers separately rather than
claiming the neighbours as a new discovery. After terminal-kind removal, the lost kind aspect disappears, but f560058d
still conditions relaunched on restart_succeeded (:549-563), and still excludes Interrupted with relaunched=false from
terminal_specs (:1702-1710 and :3701-3722), so the mounted-observer/surface aspect remains for a newer eligible listing
too.

Contract: SPEC_impl.md GUI: Dioxus says 'A successful reply supplies the terminal kind until a current detail read
catches up' and requires a mounted view to observe completion/reattachment. SPEC.md Lifecycle operations preserves
outcomes across navigation. Suggested fix: preserve completion knowledge until mounted consumers observe it, separately
from retiring the seed used by future views; merely retaining restart_revision is insufficient for surface admission.
Suggested bucket: other, unchanged. Could-be-highest: no established security or loss-of-work route. Confidence:
confirmed source-level interleaving; incidence unmeasured. proposed_drop: false. possible_cover: related interface
records g037/f3, g038/l1 and g088/l1 as distinguished above; no coverage sources searched. Caveats: a later successful
live detail normally heals the gap; no permanent outage, lost generation increment, stale-cache connected detail, or
destructive second-Restart consequence is asserted. No execution or reproduction. No remaining uninvestigated source
premise for this route.

Returned proposal (claims-021, event 7): **filtered**, suggested bucket **other**.

The source supports the transient missing-completion presentation under the named scheduling order. Its entire
established consequence is a display/attachment gap repaired by the next successful detail refresh, with no lost
process, conversation, terminal history or security route. That exact boundary matches the rare self-correcting glitch
filter.

Original stronger consequence: An already-open view can show its interrupted card or try an old terminal kind until a
successful detail read. The retained generation increment does not itself admit a terminal. Old-kind impact belongs to
the earlier protocol; no permanent outage or destructive second-Restart route was asserted.

Unresolved premise or limitation: Incidence is unmeasured. Any stronger persistent outage, destructive second-Restart
consequence or loss of user work would need its own established route and would invalidate this filter basis.

Proposed basis: Proposed filter only for the complete established transient presentation consequence; not a source
refutation or drop of separately owned stale-listing/age-of-evidence concerns.

Existing parent disposition (event 7): rejected. Parent explicitly accepted named filtered proposal from claims-021
after inspecting it. The source supports the transient missing-completion presentation under the named scheduling order.
Its entire established consequence is a display/attachment gap repaired by the next successful detail refresh, with no
lost process, conversation, terminal history or security route. That exact boundary matches the rare self-correcting
glitch filter. Proposed filter only for the complete established transient presentation consequence; not a source
refutation or drop of separately owned stale-listing/age-of-evidence concerns. Checked source/results:
["b3d6b3eff6b7508e0b1fb6ec5b5cde31d27ebabe:crates/farhelm-ui/src/session_ops.rs:92-109,219-233 lets an eligible newer
positive listing remove restart before a mounted observer acknowledges it. session_view.rs:549-564 always retains the
revision-driven generation increment but sets relaunched/terminal kind only while the payload remains.","Same
revision:session_view.rs:1252-1269 only copies selected title and requests independent detail; :1722-1730,3711-3732
excludes the interrupted terminal surface while relaunched=false. Thus generation preservation alone does not refute
admission loss.","Pinned Dioxus 0.7.10 source: use_effect.rs:15-47 queues live-reading callbacks; core
virtual_dom.rs:493-537 drains tasks/dirty work before effects; desktop webview.rs:537-590 waits for edits delivery. A
newer eligible listing can retire the success during deferred observation. This is conditional source ordering, not an
observed run.","Tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:session_view.rs:551-565 still sets relaunched only while
restart_succeeded is present. Old-terminal-kind override loss is historical to the earlier revision; the
surface-admission aspect persists."] Current coverage/results: ["SPEC.md:1901-1905 requires completion observation;
historical SPEC_impl.md at b3d6b3ef:1133-1138 promises terminal kind until current detail catches up. Current
SPEC_impl.md:1133-1140 retains mounted completion/reattachment. No explicit acceptance of the observer race.","Planned
and unlanded-plan index are empty. session-action-leftovers.md:20-70 concerns retiring new-view optimism after a
listing, and its accepted second-reboot residual is a different trigger. retained-restart-hides-reboot-card.md was read;
it concerns success retained too long after a later reboot, not removal before mounted observation. No exact
queue/active-ledger/BUGS cover found.","FILTER.md, Rare, self-correcting glitches and imprecise diagnostics, applies to
the narrow task/effect order and whole transient display/attachment consequence. The stated repair is an ordinary
successful detail refresh; none of that filter's wrong-target, persistent-record, false-success, security or work-loss
exceptions is established."] Exact cited cover: ["FILTER.md: The trigger is rare: a narrow timing window, a crash or
interruption at a specific point, or an input the shipped clients never send.","FILTER.md: The whole consequence is one
of: a display that is briefly wrong and corrects itself on the next refresh, retry or reconnect, or when the user leaves
the view; an operation that fails safely and succeeds when retried; or an error message, log line or other diagnostic
that is imprecise, misleading, missing detail, or lost."] Original stronger consequence and missing premise remain
preserved: "An already-open view can show its interrupted card or try an old terminal kind until a successful detail
read. The retained generation increment does not itself admit a terminal. Old-kind impact belongs to the earlier
protocol; no permanent outage or destructive second-Restart route was asserted."; "Incidence is unmeasured. Any stronger
persistent outage, destructive second-Restart consequence or loss of user work would need its own established route and
would invalidate this filter basis.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g036 record](records/review-through-00de0b6c/groups/g036.json). Record ID: `l1`.
