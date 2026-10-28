# Native confirmation reopening has an unresolved paint-order premise

Reviewed commit: 9b19f8d754bf7658761ef246cfda0ea8397f4f21

## TLDR

A pre-existing session Delete flyout remains focusable while its placement is unmeasured. Whether an ordinary desktop
reopen can accept a fresh answer before it becomes visible remains unverified. No destructive execution or work loss is
established.

## Details

Suggested bucket: **other**, for the conditional source lead. Confidence: **unverified after investigation**. An
established unseen destructive activation would warrant highest; the complete shipped input sequence is missing. This is
adjacent pre-existing code, not an established regression in the reviewed range:
`98bb15850f5f5ba1df6ed66873338261c45ff948..9b19f8d754bf7658761ef246cfda0ea8397f4f21` leaves the row/menu files
unchanged.

`crates/farhelm-ui/src/list/row.rs:1381–1450,1968–2070,2070–2199` resets placement and starts a new generation's
measurement when reopening; an unanswered question can remain retained across dismissal. The Unmeasured flyout is
transparent but intentionally preserves focus (`src/menu_panel.rs:145–176,193–215`). Delete uses autofocus and
`focus_confirmation`'s animation-frame focus request. That request checks disabled state, but not placement; the key
guard consumes repeated/composing Enter rather than every fresh answer (`src/lib.rs:220–245`). Busy and exact
question-token checks still apply. These facts do not themselves establish that a fresh user key can arrive before
visible placement.

The reviewer inspected installed, version-matched Dioxus 0.7.10 measurement, task, event, query, mount and edit paths.
Web geometry is read synchronously into a Ready result (`web/src/events/mounted.rs:46–61`), and immediate work/edits
flush without an intervening frame wait (`web/src/lib.rs:169–265`; `core/src/virtual_dom.rs:445–540,599–624`). The
ordinary pending web geometry-round-trip premise is therefore refuted; it is not retained here.

Native measurement awaits a Query's IPC reply (`desktop/src/element.rs:26–50,74–78`; `query.rs:43–140,152–210`). Native
events dispatch synchronously, mount callbacks occur while applying edits, and edit batches have their own
acknowledgement path (`interpreter/src/ts/native.ts:352–409,431–498,566–591`; `desktop/src/webview.rs:100–195,537–595`;
`edits.rs:45–77`). Linux's initial-hidden path uses direct edits; ordinary Mac edits use animation-frame application
(`farhelm-ui/src/desktop.rs:305–329`). The missing premise is a supported native ordering between measurement query,
transparent insertion/mount, focus, visible placement edit and a fresh ordinary Enter/Space. The inspected generic
source reaches separate platform queues without settling that ordering. A same-renderer trace showing these events,
computed opacity and actual fresh input on an ordinary retained-question reopen would settle it. No native/browser
runtime experiment ran.

This is separate from hiding host management: setup/uninstall modal siblings remain outside concealed fragments, and
that original allegation was refuted. Possible cover: SPEC.md:3550–3553 accepts a subsecond pre-update delivery window,
but its named-guard exception and the visibly focused activation rule at SPEC.md:1577–1582/SPEC_impl.md:843–850 prevent
using it as blanket acceptance. No exact Planned, unlanded-plan, BUGS, queue or ledger coverage was found. Proposed
drop: none. Do not treat the source lead as a verified highest-priority defect or silently broaden its attribution.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, independent auditor
`a002/a002-prose-hidden-confirmations`; separated from the original panel-swap candidate and narrowed through
version-matched renderer investigation during synthesis.
