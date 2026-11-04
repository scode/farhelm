# Announcement close lacks a rendered opening token

Reviewed commit: 921feddad94cd0161f41b4c0ee0778120813115a

## TLDR

The announcement close callback reads whichever opening is current instead of carrying the opening and batch the user
answered. It would dismiss a reopened history dialog if an old close input could still reach it. **That user-visible
event route remains UNVERIFIED AFTER INVESTIGATION**; only the handler shape and its render-binding contract omission
are confirmed.

## Details

Suggested bucket: `other` for the confirmed callback-contract omission. Confidence: confirmed live-state handler/binding
omission; **unverified after investigation** for an old answer closing a later history opening.
`could_be_highest: false`. `proposed_drop: null`.

At the reviewed commit, unchanged through inclusive tip `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`,
`crates/farhelm-ui/src/app_bar.rs:745–751` reads live `displayed` and `from_history`; `:762` takes the current displayed
batch, and `:766–767` persists its ids. Rendering at `:1126–1130` supplies a cloned batch beside a shared unit close
callback. `crates/farhelm-ui/src/announcements.rs:53–55,66–78,111–114` supplies neither an opening token nor a rendered
batch/mode in the answer. Help can reopen history through `app_bar.rs:1048–1052`.

`SPEC_impl.md`, “GUI: Dioxus,” “Answers bind to what was drawn,” `:1035–1064`, requires answers to act on their rendered
opening and stale-opening answers to do nothing. The source shape does not encode that binding.

The suspected user consequence requires this exact sequence: opening D0 produces two inputs A and B; A closes D0; Help
action H opens history as D1; B still belongs to D0 but is delivered after H to the live callback and dismisses D1. The
missing premise is survival or retargeting of D0-targeted B across teardown and serialized dispatch. A stable AppBar
callback alone does not establish that the removed child's event can reach it.

Cargo.lock pins the relevant Dioxus crates to `0.7.10`. The matching dependency sources show:

- `dioxus-interpreter-js/src/ts/native.ts:353–407,561–590` uses synchronous XMLHttpRequest for ordinary desktop events.
- `dioxus-desktop/src/protocol.rs:45–70` routes events inline; `webview.rs:43–107,183–201` handles the event before
  replying.
- `dioxus-web/src/dom.rs:85–122` calls the runtime directly in the browser listener.
- `dioxus-core/src/runtime.rs:359–468` resolves current ElementId/mount/listener state and ignores missing mounts.

The application has one once-per-load `read_and_show` call at `app_bar.rs:735–742`; save completion changes history only
at `:772–777`. No examined application/framework source buffers an old close answer until after Help reopening. Modal
isolation also prevents ordinary Help use while D0 is visible. These findings prevent promoting the conditional A,H,B
sequence to a demonstrated shipped event route.

Settling evidence would be an actual interpreter/native/browser trace or focused fixture showing D0-targeted B accepted
after H opens D1, with ElementId/mount lifetime recorded, or a source proof of the same target survival/remapping.
Runtime/native-event tracing was excluded by the reviewers' charter, and their inspected source did not supply that
premise. A fix direction for the confirmed contract omission is to carry opening identity and the rendered batch/mode in
close answers and reject obsolete openings.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

Grouped findings: `g036-l5` and `g037-l2` identify this same narrow concern. Neither establishes another automatic batch
replacing a visible batch, unseen-id dismissal, or draining unseen announcements. The separately recorded
`g037-l4`/`g036-l8` overwrite occurs before modal admission and does not prove stale-close delivery.

Caveats: no runtime reproduction; no unseen-batch-drain or security/work-loss consequence. The conditional event route
must remain visible in the TLDR and any priority summary. Adding a future reusable queue-read trigger would require
reassessing the batch-binding route rather than treating it as established today.

Review provenance: automated committed-code trailing review, run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`; `g036/g036-l5`,
latest log line 19, preserves its line-18 framework investigation; `g037/g037-l2`, latest line 14, independently records
matching framework checks. Earlier stronger claims at g036 line 8 and g037 line 10 are superseded as to user-visible
delivery confidence, not silently dropped.
