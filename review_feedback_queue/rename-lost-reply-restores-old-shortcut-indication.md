# A lost rename reply restores an old shortcut indication

Reviewed commit: 5fcc034722b6e24bce5e538c775fe11330fef277

## TLDR

A rename's shortcut change can commit while its reply is lost, leaving the GUI showing the old mark while the next
shortcut reads the new one. The implementation follows an explicit historical rollback criterion; its full
indication/dispatch consequences remain ambiguously covered.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/templates.rs:1844-1849`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reconciliation verdict AMBIGUOUS; retain conditional OTHER, provenance a047/f2 = g274/f1 = g273/f1. Reviewed commit
5fcc034722b6e24bce5e538c775fe11330fef277. TLDR: rename repoint can commit but lose its reply; Templates restores old A
while the next user-invoked shortcut freshly resolves saved new B. Queue false/forget route api.rs:1065-1075,1258-1267;
rollback templates.rs:1834-1849 (number analogue 1852-1868); fresh dispatch list/view.rs:3576-3606. Source mechanism
confirmed conditional on lost reply; frequency unmeasured. No harmful wrong-intent launch, confirmation bypass, security
or user-work loss is established: B is the new template the user attempted to save, and both names remain. Actual
historical plan at this revision, plans/queue/sign-in-preservation-sweep.md:83-86, explicitly permits restoring the
just-read binding on failure and says No second read. Its failure rule is unqualified, so it does cover this local
rollback when transport failure follows an unknown commit; no extra second-read requirement can be inferred from that
criterion. However, the file keeps SPEC authoritative (5-6), Goal 24-26 also says next view/save starts from helm state,
and this historical plan is landing at reviewed index:5 and absent from pinned queue, so it is not current unlanded-plan
coverage nor an explicit durable product-spec decision accepting all later stale-label behavior or definite failure
wording. possible_cover: that exact historical criterion is strong evidence of intended rollback; pinned SPEC_impl.md
says unconfirmed repoint keeps both names and shows helm mark, which does not explicitly settle unknown-commit display.
proposed_drop: historical criterion subject to adjudication; no unconditional FILTER drop. My prior whole-consequence
display-only AGREE was too definite because fresh dispatch can differ from restored indication, but mismatch alone is
not a demonstrated wrong-target/harmful operation. Keep number-key edits, rename repoints, live Boolean inversion and
post-read owner race distinct. No fix or repro recommended as an accepted requirement.

The later failed-settings-save change adds corrective rereads for a direct quick-launch mark edit. Its inspected
templates.rs diff does not change the separate rename-repoint completion branch. This record retains the rename trigger
and its exact historical rollback criterion; it does not claim the newly repaired direct mark-edit failure remains
unhandled.

Provenance: `a047/f2`, `g273/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
