# The permanent YOLO answer on a suspended host closes the question without writing or launching

Reviewed commit: e8a1719744eb1d2f426026af123ff4d58660db8c

## TLDR

When a sandbox or container host suspends while the YOLO confirmation is open, the confirmation stays up and its
one-time answer now wakes the host and launches. The "Start, and don't ask again on this host" answer, offered on the
same question, instead closes the question, saves no setting and starts nothing. At the reviewed commit it does so
silently. At the tip (after 7bef64a6c) it says "The launch changed. Review the current setup and launch again.", which
is wrong: the user changed nothing. The user has to press Launch again, wait for the wake, get asked again, and answer
again.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t10b-launcher-consent` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. SPEC.md's YOLO paragraph limits the wake carry-over to the one-time answer, which
covers the missing wake but not the silent, misleading close. No other coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:538`

At e8a1719744 the question is deliberately kept up during suspension (wake_tests.rs `answer_yolo_during_suspension`
asserts `prompt-confirm` is shown while the host is Suspended). The one-time answer goes through `launcher_owner`'s
launch closure to `submit_or_wait_for_preview` with a `YoloConsent`, and `begin_host_wake` carries `confirm_yolo`
through the wake. The permanent answer runs `stop_asking_then_launch` (yolo_question.rs ~line 489 at e8a1719744, line
538 at the tip), which first calls `owner.current`. `launcher_owner`'s `current` requires `ask.request.is_some()` and
`launcher.request(false, ..) == ask.request` (tip lines 442-443). `Launcher::request` builds the body through
`LaunchDraft::create_body`/`restart_body`/`replace_body`, which call `connected_host` (request.rs:480-484, :663), which
needs `HostStateView::Connected`. A suspended host gives `None` both in the frame (`show_request` stores `None`) and at
the press, so `current` is false. At e8a1719744 the stale branch is `attempt.succeeded(cx); return;`: the question
closes with no write, no launch and no note. At the tip, `stale_permanent_answer(.., false, ..)` (tip lines 356-387)
finds the target still matches and `refusal(cx)` is `None`, because `host_refusal` accepts a host that `can_wake`
(request.rs:455-461). So it shows "The launch changed...". SPEC.md (Starting sessions, YOLO paragraph) says the
permanent answer "first sets the host to start YOLO sessions without asking ... and then retries with the override; if
changing that setting fails, nothing is started and the confirmation stays up with the reason". Here the write is never
tried, and the question closes without a correct reason. The commit's spec text gives only the one-time answer the wake
carry-over ("A verified installation can keep the one-time answer through a wake"), so the missing wake for the
permanent answer may be intended. The silent or misleading closure is not covered by that text. To verify, read
`launcher_owner`'s `current` together with `connected_host`, then follow a test like `answer_yolo_during_suspension` but
click the permanent answer (`prompt-alternative-*`). A fix could either let the permanent answer write the setting and
then go through the same wake path, or keep the question up (or hide or disable the permanent answer while the host is
not connected) with words that say the host must reconnect first.

Suggested bucket: other. Nothing unsafe happens and no work is lost. The user has to retry and is told the launch
changed when it did not. It would move up only if the misleading text could lead a user to accept a different launch.

Confidence: confirmed (source-established for both the reviewed commit and the tip; not executed).

proposed_drop: none

possible_cover: SPEC.md YOLO paragraph limits the wake carry-over to the one-time answer; that covers the absent wake
but not the silent or misleading closure.
