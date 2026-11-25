# SPEC_impl.md says a hook setup answer waits for a checkout preview; it does not

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The implementation spec says that answering the hook-setup question, like answering the YOLO question, keeps the launch
going until a managed checkout's preview arrives. It doesn't. If the preview is still loading when the user picks Set up
and launch or Launch without, any setting the answer saves is saved, but nothing launches, and the launcher shows "Wait
for a current checkout preview before launching", so the user has to press Launch again. The text could lead a
maintainer to rely on behavior that is not there.

## Details

Concerns 7092da4d9e, which rewrote the sentence from the YOLO question alone to "either launch question".
SPEC_impl.md:1005-1006: "An answer to either launch question is accepted after its source warning is shown. It carries
that permission through a wait for a setting write or checkout preview."

Code: `crates/gui/farhelm-gui/src/launcher/view/hook_question.rs:213-233` (`continue_launch` -> `press_at`),
`crates/gui/farhelm-gui/src/launcher/view.rs:1341-1369` (`press_at` -> `submit`), `view.rs:1433-1470` (`submit`),
`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:499-528` (`submit_or_wait_for_preview`, the only place that arms a
preview wait).

Only `submit_or_wait_for_preview` sets `checkout.armed` (`checkout.rs:509-521`; there is no other `armed = Some`). Its
callers are the quick-launch path (`view.rs:420`), the wake continuation (`view.rs:3031`) and the YOLO answer
(`yolo_question.rs:193`). A hook answer goes `continue_launch` -> `press_at(Some(source))` -> `submit`. In `submit`,
`begin_host_wake` returns false while `refusal()` is Some (`view.rs:2996`), and `request()` returns None while the
refusal is `WAIT_FOR_PREVIEW` (the check at `view.rs:1384-1386`; the `refusal()` chain at `view.rs:1228-1261` ends in
`launch_refusal`, which yields `WAIT_FOR_PREVIEW` at `request.rs:427`). So `submit` sets `pressed = true` and stops. The
hook answer does carry its source warning through the setting write (`hook_question.rs:274-278`), so the first half of
the sentence holds. Before 7092da4d9e the sentence named only the YOLO question, which does arm the preview wait.

Fix: say that the YOLO answer carries its permission through a host-setting write or checkout preview, and the hook
answer through its setting write. Alternatively, if hook answers should wait, route `continue_launch` through
`submit_or_wait_for_preview` with the answer's source.

Bucket: other (a spec misstatement whose shipped behavior fails safe: no launch, and a refusal shown). Not highest: the
behavior refuses rather than acting on unshown state, and the misstatement cannot lead to a security or data-loss
mistake.

Confidence: confirmed from source, assuming `continue_launch` runs while the managed-checkout preview is still Waiting
or Failed. Nothing prevents that: the hook question is asked on explicit agent selection, independent of the preview.

Possible cover: none known. SPEC.md:3196-3200 says only "The first saves the setting before continuing the launch."

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g17 record f2; claude-opus-5-5
reviewer, source analysis only, nothing executed.
