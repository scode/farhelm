# A stale "don't ask again" YOLO answer closes the question with nothing started and no reason

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When the launcher's YOLO confirmation is open and the launch it asked about changes underneath it (the host reconnects,
say, or the user opens Save as template), pressing "Start, and don't ask again on this host" can make the confirmation
simply disappear. No session starts, the host setting the user asked for is not saved, and the launcher says nothing
about why. If the change happens while the setting is being saved, the host has already been switched to start YOLO
sessions without asking, yet still nothing starts and nothing tells the user; their next Launch then starts a YOLO
session with no question at all.

## Details

Concerns e7a61fcfe0 ("fix: guard delayed consent in the session launcher"). Locations:
`crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:385-389` (pre-write staleness check) and `:423-432`
(post-write check); the launcher owner's `current` closure at `:281-293`.

The commit adds two gates on the permanent answer. Before the host write:
`if !(owner.current)(&ask, &attempt, cx) { attempt.succeeded(cx); return; }`. `Attempt::succeeded` is the prompt's
success path, which closes the question "as any answer does" (its docs in `prompt.rs`). After the write succeeds:
`let current = (on_reply.current)(&answered, &attempt, cx); attempt.succeeded(cx); if current { launch }`, so again the
question closes whether or not anything launches. For the launcher owner, `current` is false whenever
`launcher.request(false, source, cx)` differs from the request captured at the last frame (Render in `view.rs` calls
`YoloState::show_request` every frame), so any store change that alters the request body (host incarnation after a
reconnect, a refusal appearing because the host went offline, a recipe or checkout preview change) between the shown
frame and the click, or during the write, takes this path. Neither path sets a note, refusal, notice or failure line;
`self.yolo` keeps a closed Inline, so nothing else is drawn in its place (`yolo_inline` in `yolo_question.rs`; the
notices region in `view.rs`). While the closed question is held, it also takes the notices slot the refusal alert would
otherwise use (`view.rs:2771-2790`, `question.or(alert)`), until the next press clears `self.yolo`. The new test
`a_stale_permanent_answer_does_not_write_after_reconnect` (`yolo_tests.rs:357-392`) asserts exactly this silent close
("the stale answer closed").

A route that needs no reconnect: with the YOLO confirmation up and no write out, Save as template stays available
(`save.rs:39` refuses only while `yolo_in_flight`), and opening it does not retire the question (`open_save_panel` never
calls `changed`). With the panel open, `Launcher::refusal` returns `TEMPLATE_PANEL_OPEN` (`view.rs:1235-1237`), so
`request()` is None, the next frame binds `ask.request = None` (`view.rs:2324-2325`), and `current` requires
`ask.request.is_some()` (`yolo_question.rs:290`). Pressing the permanent answer then takes the pre-write stale path: the
question vanishes, the host setting the user asked for is not written, and no reason is shown. The one-off answer in the
same state goes through `submit` and shows the ordinary refusal "Finish or cancel saving the template before launching",
so the two answers disagree. Before e7a61fcfe0 the permanent answer wrote the setting and then its launch was refused
with that same visible reason. A host going offline, or a host probe, setup or update in flight elsewhere
(`view.rs:1229`), has the same effect.

Compare SPEC_impl.md's prompt section (around line 1044): an answer that arrives before a frame showing a reworded
question "does nothing", that is, the question stays up for a fresh answer; and the failure path of the same answer
keeps the question up "with the reason" (SPEC.md as quoted at SPEC_impl.md around line 1052). SPEC.md:1596-1599
("Creation") says the permanent answer "first sets the host to start YOLO sessions without asking ... and then retries
with the override; if changing that setting fails, nothing is started and the confirmation stays up with the reason".
The commit adds a third outcome (setting written, no retry) and a fourth (nothing written, no retry, question closed)
that the spec does not describe, and did not update SPEC.md or SPEC_impl.md for them.

Fix: on the pre-write stale path, keep the question open (for example `attempt.failed(...)` with a sentence saying the
launch changed and to answer again, or leave the attempt unanswered so the refreshed question can be answered); on the
post-write path when not current, close the question but show a launcher note that the host now starts YOLO sessions
without asking and that the launch was not started because it changed.

Bucket: other (a confusing silent outcome; the setting written is the one the user asked for, and no unconsented launch
happens). Not highest: no launch happens without consent, and the follow-up unasked YOLO launch is exactly what "don't
ask again" authorizes.

Confidence: confirmed from source (both branches call `attempt.succeeded` with no message; the commit's own test asserts
the close).

Possible cover: none known; the YOLO paragraph in SPEC.md's "Creation" says a failed stop-asking write keeps the
question up with the reason, but says nothing about a stale answer.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g18 record f2; claude-opus-5-5
reviewer, source analysis only, nothing executed.
