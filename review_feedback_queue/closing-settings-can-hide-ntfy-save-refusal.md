# Closing Settings can hide a failed ntfy save

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

If you save an ntfy setting and close Settings before the helm refuses the write, the window can lose the refusal.
Reopening shows the old value with no explanation.

## Details

Bucket: **other**. GUI migration final whole-tree review, R8 Sol closed Settings save refusal.

Source references: `crates/gui/farhelm-gui/src/settings.rs:260, 810`, `crates/gui/farhelm-gui/src/drafts.rs:155`,
`crates/gui/farhelm-gui/src/operations.rs:889`, `crates/gui/farhelm-gui/src/settings/ntfy.rs:390, 645, 727`.

The Settings dismissal check only guards the recipe editor. The ntfy form's shared draft helper tries to upgrade a weak
reference on completion, but closing Settings drops that form. The refusal remains on the generic preferences operation
key. The ntfy error line comes from the draft's refusal (line 727, as corrected by the reviewer), so a new form never
shows it.

Sol rated this source-confirmed and noncritical. There is no separate verification verdict. SPEC.md “Errors and
diagnostics” and “One GUI at a time” require the failed explicit save to be reported, and ntfy is outside the named
best-effort exceptions. FILTER.md's rare diagnostic filter may cover an exceptionally timed refusal, but there is no
sub-second deadline for the user to close a slow save.

Publish the explicit save's failure somewhere that survives form dismissal, or defer dismissal until the save can report
its result. Verify a refused save delivered after Settings closes while the channel stays connected.
