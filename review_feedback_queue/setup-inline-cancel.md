# Cancelled host setup can turn off future setup questions

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Cancelled host setup can turn off future setup questions.

## Details

`F23 / COR-SETUP-INLINE-CANCEL` — **definite** — `crates/farhelm-ui/src/provisioning.rs:2749` — Cancelled host setup can
turn off future setup questions

Cancelling setup on an existing host stops the immediate install, but a queued “yes, and don't ask in the future” answer
can still save that permanent preference. The answer handler changes the shared preference and schedules its durable
write before checking whether there is still a setup plan to confirm. Cancel has already cleared that plan, so the
install path returns without submitting anything while the preference change survives.

The user has cancelled setup yet may find that later host additions install without the expected question. Consume and
validate the live plan bound to this question before recording the permanent answer. A regression should show that
Cancel followed by the permanent answer changes neither the preference nor sends a setup request.

Suggested bucket: high

Possible cover: none

Caveats: No browser reproduction was performed. The immediate setup remains stopped. The lifecycle reviewer considered
this potentially highest priority because it changes durable consent behavior; the original high bucket is preserved.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **high**.

Originating reviewers and passes: `ui_data p1`, `ui_lifecycle p1`.

Possible cover recorded during collection: none identified.

Collection caveats: No browser repro, immediate setup remains stopped. Lifecycle reviewer conservatively possibly
highest; original high bucket preserved.

## Filed reviewer metadata

- `ui_data p1`: confidence as filed: **definite / confirmed**, under the same queued-event premise. Suggested bucket as
  filed: **high**.
- `ui_lifecycle p1`: confidence as filed: definite; confirmed by preference write preceding the live-plan check. Caveat:
  not runtime reproduced. Suggested bucket as filed: high; potentially highest because it removes later remote-setup
  consent.
