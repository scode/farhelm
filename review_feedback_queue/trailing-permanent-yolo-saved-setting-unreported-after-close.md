# Closing the launcher after an edit during the permanent YOLO write leaves the saved setting unreported

Reviewed commit: 7bef64a6c450ffc98f27b66902b33bdb9175c204

## TLDR

A user presses "Start, and don't ask again on this host", then changes any field while the host setting is still being
written. Changing the field retires the question, which releases the launcher's "hold still" guard, so Escape or Close
now closes the launcher. The write then succeeds and the host starts YOLO sessions without asking from then on, but
nothing tells the user: the new "now starts YOLO sessions without asking" notice this commit adds goes to a launcher
that no longer exists. The last thing the user saw was a question saying "Nothing has been started", and they closed the
form believing they had backed out.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t01g-launcher-custom` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept; the reviewer notes it would move to `high` if the maintainer treats the missing
disclosure of a consequential, security-relevant setting as material. The user did ask for the setting, so nothing
happens that they did not authorize. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:284`

All line numbers are against 7bef64a6c4.

- `launcher/view.rs:1040-1046` (`changed`): every edit sets `self.yolo = None` (and `yolo_setting_saved = None`).
- `launcher/view/yolo_question.rs:285-288` (`yolo_in_flight`): it reads the write's in-flight state only through
  `self.yolo`, so it returns false once an edit has cleared it, even though the host write is still out.
- `launcher/view.rs:233-252` (`on_dismiss`): it returns `Dismissal::Stay` only while `yolo_in_flight`,
  `hook_write_in_flight` or `template_saving` is true, so Escape or Close now closes the dialog. The layer owned the
  launcher view, and `Shown` keeps only a weak handle (`launcher/entry.rs:164-170`), so the launcher goes away.
- `yolo_question.rs:576-593`: when the write succeeds, `current` is false, so the code calls
  `on_reply.stale_permanent_answer(&answered, true, cx)`. That function (`yolo_question.rs:356-387`) does
  `let _ = launcher.update(...)` on the `WeakEntity`. On a dropped launcher this does nothing, so `yolo_setting_saved`
  is never set or drawn anywhere.

This commit adds the spec promise (SPEC.md, the YOLO paragraph under Creation): "once the host has saved the setting, it
says that the host now starts YOLO sessions without asking, beside any newer question or refusal, until the next edit or
launch". The existing test docstring (`launcher/yolo_tests.rs`, `the_launcher_holds_still_while_stop_asking_is_out`)
gives the reason for the hold: "closing the launcher would let the write change the host's setting with nothing left to
report it". The hold covers that case only until the first edit. The commit's own test edits the name during the write
and keeps the launcher open, so it never exercises the close.

How to verify: extend `the_launcher_holds_still_while_stop_asking_is_out`: after the name edit, press Escape (or click
Close), then reply 200 to the write. Today the launcher closes and no surface mentions the saved setting.

Possible fixes: (a) keep the launcher from being dismissed while any permanent write it started is still out, by
tracking that write apart from `self.yolo` (for example, a pending flag or the op key kept until the reply arrives); or
(b) when the launcher is gone, report the saved setting somewhere that outlives it (a window notice).

Suggested bucket: other. The user explicitly asked for the host setting, so nothing happens that they did not authorize;
the harm is a missing disclosure of a consequential, security-relevant change. Move to high if the maintainer treats an
unannounced switch to "YOLO without asking" after the user apparently backed out as a material trust problem.

Confidence: confirmed (source-established route: edit clears self.yolo, yolo_in_flight becomes false, on_dismiss closes,
and the weak update in stale_permanent_answer has no visible target).

proposed_drop: none

possible_cover: none known; the spec sentence quoted above promises the notice but does not address a launcher closed
during the write.
