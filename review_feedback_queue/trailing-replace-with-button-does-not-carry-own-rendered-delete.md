# Replace with button does not carry its own rendered Delete guard

Reviewed commit: 19f967247150131de3b37127df7f028669ffe269

## TLDR

The main Replace with button reads the most recently presented source warning when its callback runs, so the button
itself carries no guard from the frame that made it available.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`crates/gui/farhelm-gui/src/launcher/view.rs:2703`.

launcher/view.rs:2684–2703 installs the generic press handler. That handler reads source_frame.current() at 1296–1316.
The renderer computes and captures the source guard at 1242–1256 and 2151–2156. prompt.rs:1176–1215 rejects a newest
frame that has not been presented, but dereferences the latest stored frame for this button. SPEC.md:2170–2175 requires
'the precondition matching what the launcher showed at the click'; 6ef88c9a:SPEC_impl.md:1551–1574 requires the
handler's own rendered data. A keyboard shortcut without its own element can use the currently presented snapshot. A
pointer button can carry the ShownAction created for its frame. The deferred YOLO paths preserve the guard from their
answer through the host-setting response or the pinned preview (view/yolo_question.rs:267–274, 312–320, 355–388.
View/checkout.rs:506–549). Existing tests in launcher/replace_with_tests.rs:289–375 assert the current rendered guard
and a delayed setting reply preserving the confirmed guard. The source-level pointer binding gap is confirmed.
Unverified after investigation: a delayed pointer event actually using a newer permissive guard and stopping processes
the user did not consent to stop. That requires source proof that the shipped GPUI pointer dispatch can invoke the
earlier button after a more permissive snapshot is presented, with the resulting weaker guard accepted on the same
source. The inspected render, guard tests and supervisor checks at service/core.rs:13404–13503 did not establish that
premise. Bucket: other for the binding gap. Highest only if the destructive route is established. Confidence:
source-supported binding gap, no observed wrong dispatch. Possible coverage:
replace_with_follows_the_source_and_sends_the_guard_it_shows and
replace_with_warns_about_a_running_source_and_sends_its_guard cover current frame and delayed settings response, not an
earlier pointer callback.

Independent check of the proposed exclusion. DISAGREE via 19f9672:launcher/view.rs:2682–2705, prompt.rs:1174–1203,
SPEC.md:2168–2177; 6ef88c9a:SPEC_impl.md:1549–1578; 74b81e5:FILTER.md:254–267. The launch button confirms deletion, yet
its click handler carries only the launcher handle and generic press method. The requirement to bind the answer to its
own render makes that an independent contract gap worth retaining in other. A delayed click reaching a weaker guard and
stopping additional processes remains unestablished.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `checkout/l15`.
