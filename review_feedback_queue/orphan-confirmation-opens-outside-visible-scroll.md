# Orphan confirmation opens outside visible scroll

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

With a long orphan list, the new confirmation can open outside the visible area without scrolling or moving focus to it;
its controls remain reachable by scrolling.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/tensorlake/orphans.rs:142-181 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g074/f10, event 30):

Reviewed commit 8bd92e1336565dfc263fe5f3fa2fef413a82a87a. TLDR: with enough sandboxes in Settings, clicking Delete or
Retry cleanup on an early row can appear to do nothing because the new question and its answers are appended below all
remaining rows and are not brought into view. Source route: OrphanList :142-162 renders every returned row in normal
flow; :153-154 only sets confirm, leaving the clicked row/control in place. The question and both answers are appended
after that entire list at :163-181. No focus/reveal/onmounted effect is present in the complete component. Checked
shared wrappers: modal_dialog.rs:68-75 DialogActions only supplies a div and spacing, no handlers or focus. Checked
pinned CSS: app.css:3752-3753 templates-departure is an ordinary grid, :4150 host-settings-section is a grid with 14px
gaps, :4276-4278 answer rows are flex spacing, and no orphan-delete-confirm-specific positioning exists. Settings body
is the bounded scrolling dialog-body (settings.rs:219-220,324,356; app.css:3311-3319 max viewport height, :9758-9765
overflow:auto). Settings' explicit scrollIntoView at settings.rs:329-339 is only its initial trust-band entry point, not
an orphan-confirmation reveal. Thus enough normal-flow rows separate an early clicked row from the appended question
outside that finite scrollport, with no code to move focus or scroll to the answer. SPEC.md Topology at pinned tip
:393-397 promises Delete asks plainly before cleanup; this arrangement makes the asking hard to discover for long lists.
Suggested fix render each question adjacent to its selected row, or explicitly reveal/focus the question/answers when
opening; preserve the token-aware authority fix already recorded as f1. Suggested bucket other,
discoverability/convenience only; no accidental deletion/data-loss claim. Confidence source-confirmed placement and
missing reveal under long-list premise; no dynamic browser/layout reproduction, no exact row/viewport threshold
measured. Caveats: answers remain reachable by scrolling or sequential keyboard navigation, and role=status can announce
the question to assistive technology; short lists are unaffected. This is distinct from already recorded confirmation
lifetime, error suppression, missing running status and refresh generation mechanisms. Initial
c340166d754d3edb3de7fd538c6408948d3536c6 UI likewise appends confirmation after rows (:97-138); assigned refresh patch
retains it, prior path history shows no later repair. proposed_drop false; possible_cover none among supplied interface
records, coverage sources not searched. Discovery pass 3 count one novel source-verified finding f10, zero
investigated-unverified. Lead f10 resolved.

Later qualification (review record g074/f10, event 34):

Completion evidence for f10, not another count. Reviewed 8bd92e1336565dfc263fe5f3fa2fef413a82a87a. TLDR remains
long-list Delete/Retry opens its question below the remaining rows without revealing it. Checked pinned tip app.css
selectors: templates-departure :3873-3874 remains a normal grid; host-settings-section :4271 remains a normal grid;
host-dialog-answers :4398 remains flex spacing; recipe-only sticky rules :4120-4121 do not apply to the orphan
confirmation, and no orphan-delete-confirm positioning rule exists. Tip dialog-body remains the scrolling body at :9918
onward. Current OrphanList's unchanged 142-181 placement and lack of reveal were directly read in the preceding pass;
the shared wrapper remains spacing-only at reviewed modal_dialog.rs:68-75. Suggested bucket other; confidence
source-confirmed under long-list finite-scrollport premise, no dynamic layout reproduction or threshold measurement.
proposed_drop false; possible_cover none selected. Actions remain reachable by scrolling/Tab and short lists need not
exhibit it. Initial complete f10 event supplies behavior, constraints, spec and remedy. Lead f10 is resolved and no
additional premise is left open.

Recording coverage: Dialog explanations reachable by scrolling is close, but the finding concerns reveal/focus of the
newly opened confirmation controls themselves. The exact filter boundary is ambiguous; retain.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g074/f10`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
