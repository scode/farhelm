# A queued close can discard feedback while sending starts

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

A queued close can discard feedback while sending starts.

## Details

`F50 / COR-FEEDBACK-QUEUED-CLOSE` — **possible** — `crates/farhelm-ui/src/feedback.rs:288` — A queued close can discard
feedback while sending starts

A close event queued with Send may discard the feedback draft while delivery is still unresolved. Send immediately
records that sending has begun, and guards against a second send by reading that live state. Cancel instead calls the
parent's close handler unconditionally. Escape checks the earlier render's sending flag, which can still say false
before the next render disables the controls. The modal's own Escape fallback likewise sees Cancel as enabled until that
render reaches the page.

If either close path runs in that interval, it can unmount the dialog, drop its send task, and lose the typed text
without establishing whether delivery succeeded. The product specification requires a failed send to retain the text,
and the dialog's own comment explicitly says that closing during a send must wait. Have both close handlers read the
live sending state before closing, and exercise Send followed by Cancel or Escape in a controlled same-task sequence.

Suggested bucket: highest

Possible cover: none

Caveats: The project establishes that queued events may arrive before a render, but this exact Send/Close browser
ordering has not been executed. The finding remains possible. Native disabling is insufficient during the interval
before the next render.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `ui_systems p2`.

Possible cover recorded during collection: none identified.

Collection caveats: Eventburst contract sourceestablished but exact browserordering unexecuted; modal fallback
nativeenabled untilrender.

## Filed reviewer metadata

- `ui_systems p2`: confidence as filed: possible; likely. Material premise: the renderer can deliver Send followed by
  Cancel/Escape before applying the next render. The codebase explicitly assumes this ordering for the same dialog's
  duplicate-Send protection (:173–177), and its session launcher separately guards queued Cancel for this reason
  (list/create_form.rs:4011–4018). This exact feedback event ordering was not executed during the read-only review.
  Suggested bucket as filed: highest (loss of the user's typed feedback and unknown delivery).
