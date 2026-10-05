# Feedback forwarding can be cancelled with its HTTP request

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Feedback forwarding can be cancelled with its HTTP request.

## Details

`F1 / COR-FEEDBACK-CANCEL` — **definite** — `crates/farhelm-helm/src/feedback.rs:172` — Feedback forwarding can be
cancelled with its HTTP request

After validating a feedback submission, the helm sends it to the feedback service directly from the task handling the
browser’s HTTP request. The web framework can drop that task when the requester disconnects. Closing or reloading the
page while the send is pending can therefore stop forwarding before the feedback service accepts the submission.

Farhelm’s implementation specification requires accepted state-changing work to belong to the server: losing the
requesting connection should lose only the reply. The feedback handler does not use the helm’s existing mechanism for
that ownership. Once validation succeeds, move forwarding onto a helm-owned task and let the HTTP handler wait for its
result. A controlled cancellation test should hold the outbound send pending, drop the requester, and verify that
forwarding still completes.

Suggested bucket: **high**. No possible cover was identified. This is established by source inspection and the
documented cancellation contract, without a runtime reproduction.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **high**.

Originating reviewers and passes: `helm_general p1`.

Possible cover recorded during collection: none identified.

Collection caveats: No runtime repro; axum cancellation contract explicit lib2093 and SPEC.

## Filed reviewer metadata

- `helm_general p1`: confidence as filed: definite / confirmed by code and contract, without runtime reproduction.
  Suggested bucket as filed: high, material UX. Confidence: definite / confirmed by code and contract, without runtime
  reproduction.
