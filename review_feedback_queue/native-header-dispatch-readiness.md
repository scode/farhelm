# An early native header action may precede dispatch readiness

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other, unverified after investigation. The
environment rewrite preserves the exact readiness mechanism: `session_ops.rs:146` initializes None, `229–234` ignores
sends while absent, and `list/view.rs:3450` installs the callback in `use_effect`. Initial listing still starts
independently in a mount hook (`list/view.rs:2512–2518`). It adds environment actions and claim semantics, without a
first-paint readiness barrier. No exact later repair or cover was found. The safe-retry filter remains possible but
ambiguous. Automatic-selection/browser refutations, the refuted tracked-write-starvation premise and the missing
complete physical native schedule below remain unchanged.

## TLDR

A very early native desktop header action might silently do nothing if a manually opened session becomes interactive
before its action dispatcher is installed. That physical input/render ordering remains unverified after investigation;
ordinary automatic startup selection and the browser path were refuted for this hypothesis.

## Details

Bucket: **other**. Confidence: **unverified after investigation**. The reviewer suggests other, with confidence only in
the conditional no-op mechanism. No dropped physical action was observed, and no destructive, security or user-work-loss
consequence is claimed. Same-reviewer and interface source checks narrowed the missing schedule without establishing it.

Reviewed source change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`. At the reviewed pin,
`crates/farhelm-ui/src/session_ops.rs:58,72` initializes `dispatch=None`; `send` at 118-123 simply returns without a
request or diagnostic when that value is absent. `list/view.rs:3172` installs the list-owned callback from `use_effect`
(3071 in the original source change). `lib.rs:1919` starts with no selected session, so no header exists in the first
tree.

The automatic-selection effect is declared after dispatcher installation. Matching Dioxus 0.7.10 queues same-scope
effects FIFO and drains their callbacks synchronously (`effect.rs:17-36`, `tasks.rs:214-250`), so ordinary automatic
selection installs the dispatcher before creating a header. The browser renderer processes effect work after flushing
edits and before yielding to another physical event; a generic browser lost-action route is not retained.

The remaining suspicion requires two manual boundaries before the first effect batch: a listing becomes visibly usable,
the user selects its row, that selection paints a header, and a header activation reaches Rust before dispatch installs.
The initial listing is absent; its mount read starts independently from `use_hook` (`view.rs:2296-2301`,
`reader.rs:644-678`), and publication at `view.rs:2111-2121` can dirty rendering. Dioxus core polls tasks ahead of
effects and may render on newly dirty scopes (`virtual_dom.rs:494-541,599-625`). First-row paint is therefore not a
universal proof that the installer already ran. Conversely, once this scope's effect batch begins, it drains FIFO
without a scheduler interruption; another native event cannot interleave between the installer and its later
automatic-selection effect.

Desktop rendering waits for DOM-edit acknowledgments before effect polling (`webview.rs:537-590`, `edits.rs:54-74`),
while the HTML event route calls `runtime.handle_event` independently (`webview.rs:190-194`). That does not establish
actual startup starvation or the complete physical input/render sequence. A pending network read ordinarily allows an
idle effect drain. The intermediate argument that the reader's pre-await `state.write().begin` necessarily dirties a
render was refuted: `dioxus-signals` 0.7.10 notifies tracked subscribers only (`signal.rs:440-460,527-538`), and no
production tracked render subscription to those listing/host surface signals was found. Request plumbing and peek-only
test snapshots do not supply one. Directly awaiting the HTTP client and a multithreaded native runtime likewise supplied
no actual first-response-before-installer witness. Do not use those observations to manufacture the missing schedule.

The missing premise is a supported ordering among native edit acknowledgments, Poll wakeups and physical input producing
those two usable render boundaries before any idle point drains installation. A trace logging initial listing
completion, row/header paints, manual inputs and installation, or a renderer ordering guarantee covering them, would
settle it. No renderer operation or runtime experiment ran under the source-only review charter. Do not infer a physical
lost action merely from `use_effect` being deferred.

If reachable, an action exposed as available should dispatch or visibly refuse rather than disappear. `SPEC.md`,
“Lifecycle operations”, defines the header operations; “Errors and diagnostics”, 4239-4241 requires a concrete client
error for a failed operation. The delivered-input allowance at 5017-5020 does not expressly accept silently losing an
ordinary activation to an uninitialized callback. A fix would establish dispatcher readiness before exposing these
actions or give that not-ready path an explicit outcome, without adding a queue based only on this suspicion.

Recording-time coverage was checked at main `cb03def00870e18a98a1fd9046073d2849be3253`. No exact queue, current ledger,
Planned item, indexed unlanded plan or BUGS entry covers this initialization boundary. The startup-selection item is a
different delayed preference-read/navigation route. Possible filter cover remains: “Rare, self-correcting glitches and
imprecise diagnostics” covers a narrow-window action that fails safely and succeeds on retry. If the suspected exposure
is such a narrow startup window, the consequence could fit; the necessary native ordering and timing bound are not
established, so a full match was not concluded. Preserve that possible cover for triage rather than implying material
outage or normal startup failure.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `native-header-dispatch-readiness`; `g006/l6` through
event32 and alias `g007/g007-d1` through event23 in `prep/groups/<group>/findings.jsonl`. g007 revised its earlier broad
candidate exclusion to retain only this native startup uncertainty; installed-owner lifetime, automatic-selection and
browser refutations remain valid. Supporting checks are `g008/g008-c10` event38, `g009/d5` event25 and `g013/c11`
event29, not additional defects. These source checks establish the bounded uncertainty described here; they do not
establish a physical lost action.
