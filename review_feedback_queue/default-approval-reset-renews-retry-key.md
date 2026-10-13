# Default-approval templates lose their retry identity

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

A template resetting approvals to the agent's default can make YOLO confirmation repeat indefinitely. On a host that
already permits approval-free launches, retrying an unanswered launch can start a second session.

## Details

Suggested bucket: `high`. Confidence: confirmed static trace; neither the browser confirmation loop nor lost-response
duplication was executed. Source: adversarial launcher reviewer. Proposed drop: none.

This mismatch and its template-reset route already exist at v0.25.0. It is a surviving pre-existing bug found in the
retry path changed by `e64574a9158d1aec5ef07461221835e9b6ea950e`, not a newly introduced regression.

Apply an OMP or Goose agent template containing `"permissions": null`. Protocol template application accepts that reset
(`crates/farhelm-proto/src/launcher.rs:621–631`), and the UI applies the default-permissions action
(`crates/farhelm-ui/src/list/create_form.rs:515–521`), which stores `None` (`794`). No effect continuously normalizes
that signal, and catalog compatibility accepts omitted permissions (`launch_composer.rs:1361–1363`).

Submission normalizes that default to effective `Some(Yolo)` (`create_form.rs:4139–4147`). Its first mint binds K1 to
the normalized selection, but the post-await recheck reconstructs raw `None` (`4462–4469`). The mismatch causes another
mint K2, now bound to raw None. An unchanged retry starts normalized again, does not match retained K2, and mints new
keys. Thus the failure-key retention (`4702–4716`) cannot provide deduplication for this input.

On a host requiring YOLO consent, the question names K2 (`4670–4684`). Confirming submits again and remints; the new key
does not equal the confirmed key when `allow_yolo` is computed (`4518`), so another question appears. On a permissive
host, an accepted create whose reply is lost can instead be repeated under a different key.

Normalize the post-mint reconstructed selection with the same helper used by initial submission. A focused browser route
fixture should establish that a null-reset OMP/Goose template completes consent and retains one request key across
unchanged retries; separately verify one session after dropping an actual successful response.

Possible cover: `TRIAGE_OUTCOMES.md:10396–10419`, `retrying-through-unchanged-permission-choice-launch-duplicate.md`,
addresses unconditional key clearing by choice callbacks. Its trigger differs from this normalization mismatch; the
completed callback fix does not cover it. No exact other queue item or accepted decision was found.
