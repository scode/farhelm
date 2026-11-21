# Loss of an upload notice before its first presentation is unverified after investigation

Reviewed commit: 9864b874b3ac36670cb52a566ecbc08610a71430

## TLDR

Another file offer might clear an interrupted-upload notice before it first appears. The platform event ordering remains
unverified.

## Details

**other**. Unverified after investigation.

Unverified after investigation. When a hidden-session visit is showing Agent and still awaiting its row, a previously
interrupted upload's notice can first be transferred into Agent when the user clicks the already selected Agent tab. If
a file offer is handled before that notice is presented, the earlier outcome can be permanently removed from this view
before the user sees it. Confirmed code paths: the rowless pane can still show the primary tab
(session_pane/header.rs:558–565,614–620. Terminal/set.rs:673–698). Selecting it clears restore and takes carry
(set.rs:486–500,618–628). Attachments.rs:695–706 removes the only global copy and requests redraw. A file offer targets
the same terminal through an asynchronous clipboard result or its drop handler (attached.rs:1624–1683,2435–2439). Both a
live upload and a not-connected offer clear earlier lines without a visibility gate (attachments.rs:496–503,542–551).
GPUI turns a native file drop into a mouse release and dispatches against its rendered frame
(vendor/gpui-pre/src/window.rs:6160–6177,6214–6219,6423–6457). The app exposes an explicit presentation acknowledgement
in prompt.rs:1149–1157, with no equivalent in these attachment paths. The parent pulled existing carried notices into
Agent as it was constructed (parent attachments.rs:467–497), so it did not defer their first presentation until the
Agent tab click in an already displayed pane. Missing premise: whether a pending clipboard result or fresh platform drop
can reach this live or refused offer path before any successful presentation of the late carry in the shipped event
order. Separate source callbacks and absence of a guard establish the dependency, but the checked source has no observed
or forced production ordering. A platform event/presentation timeline or existing deterministic coverage would settle
it. No execution allowed or performed. Estimate other, with no loss of the local file or running process established.
SPEC.md:3461–3463 requires visible failure. SPEC_impl.md:486–493 describes carrying it to the shown terminal.
Possible_cover: attachments.rs:496–498 says a new offer supersedes a line on screen, without addressing a previously
unpresented line. This is different from tabs/f2's retained notice awaiting any delivery.
A315f83db44cf741961ff7ed79712b92755fd0f1 is prose-only and the pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f
preserves these transition paths.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tabs/f12`.
