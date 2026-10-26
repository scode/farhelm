# A pending switcher read keeps old shortcut labels

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

## TLDR

A successful but still-pending opening read can leave old shortcut labels visible in the switcher. Existing decisions
accept failed reads and later moves, but this interval's coverage is ambiguous; selection still captures the displayed
template name.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/quick_switcher.rs:219-234,246-277,372-405`. Locations and revisions in the
assessment below identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit 463cac1528b5c5e259404975380d321a3fff0412. Verdict remains AMBIGUOUS for exclusion of the pending
successful opening preference read. Cached labels can accompany rows before this independent read completes; failed
reads and moves after opening remain the only exact accepted variants in SPEC_impl.md:1166-1169 and
TRIAGE_OUTCOMES.md:16374-16400. Synthesis confirms the overlay is applied after completion (api.rs:1120-1133,1322-1326;
list/templates.rs:49-54), not a readiness dependency for rows. The unit test quick_switcher.rs:497-531 proves settled
binding replacement and unrelated preference preservation; it does not establish pending-display acceptance or a
rendering gate. The inspected browser exact-name test template-list-shortcut.spec.ts:42-94 seeds identical cached/server
bindings before page load, so it does not distinguish the pending stale-label case. A picked name survives modal removal
as Name(name), TemplateList origin, in list/view.rs:3770-3815; this is a non-keyboard origin and uses fresh context
rather than the keyboard snapshot FIFO. No source-established harmful-launch premise was added. Suggested bucket other;
confidence confirmed conditional display mechanism, ambiguous acceptance. Frequency/duration and native/browser
interleavings unmeasured. possible_cover: historical badge ledger and explicit failed-read/later-move clauses, partial
only. proposed_drop false for pending success; true only for those exact accepted variants. No new defect is inferred
from tests lacking this case, and no runtime work was performed.

Provenance: `a031/f2`, `g036/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
