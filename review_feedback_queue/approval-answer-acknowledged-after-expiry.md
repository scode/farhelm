# An approval answer can disappear without taking effect

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

If you answer as an approval expires, Farhelm can remove the card without telling you that the agent's request expired.
An Always allow setting can still have been saved.

## Details

Bucket: **other**. GUI migration final whole-tree review, R3 Sol answer at expiry.

Source references: `crates/farhelm-helm/src/approvals.rs:533–537, 682–696, 876`,
`crates/gui/farhelm-gui/src/approvals/answers.rs:200`.

The request's waiter selects between the answer and the deadline. When the deadline wins after the answer handler takes
the pending entry but before the waiter receives the answer, the ordinary handler ignores its send result and
replies 204. The GUI only shows the late-answer notice on 410. Host Add already uses a receipt to make acknowledgment
follow the actual decision.

Sol rated this source-confirmed and noncritical. There is no separate verification verdict. SPEC_impl.md's approval
rules accept the permission-setting race and call for a 410 plus a notice when the request expired, including the
distinction between the setting and the request. The existing answer semantics for ordinary actions are mentioned there.
FILTER.md's rare diagnostic rule may cover the missed notice alone, but explicitly excludes reporting success for a
failed operation.

Acknowledge after the waiter accepts an ordinary answer, returning Gone when expiry won. Test a controlled deadline and
check for the late-answer notice, including the separate saved grant.
