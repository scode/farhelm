# Applying Guarded permissions leaves the old YOLO question

Reviewed commit: 6d0b152493a31bcde6af09f149cc9f5a83287b0f

## TLDR

After a launch asks for YOLO approval, applying a Guarded template can leave the old question visible. Its permanent
answer advertises a host-policy change that the changed launch will not perform. Submission still uses Guarded
permissions.

## Details

`crates/farhelm-ui/src/list/create_form.rs:801–805` applies Guarded, but the change restores the previous retry pair
(`733,883–887`) even when the effective request changes. The question render checks only that retained key against
`refused_key` (`5994–6003`). Same-host/harness changes leave the key and old question (`3365–3373,4014–4047`). The
captured YOLO explanation and permanent answer remain (`yolo_confirm.rs:251–284`). Invalidate the question when its
refused binding differs from the live draft while preserving unchanged-template retry recovery.

Submission compares the changed request and mints a fresh key (`create_form.rs:5414–5432`), then rejects old consent and
stop-asking authority (`5539–5548`). Host-target changes separately invalidate. No permission bypass, wrong-host policy
change or user-work loss was established; the retained finding is presentation and an ignored advertised action. Fresh
retry-binding review independently supports the typed equality and consent boundary.

Suggested bucket: **other**. Confidence: **confirmed static question/dispatch mismatch; browser behavior unexecuted**.
Proposed drop: none. The rare-diagnostic filter has a recoverable consequence but no rare trigger: this is an ordinary
explicit template edit, without a narrow race or crash.

Possible cover: `SPEC.md:852–855` preserves unchanged retries; `857–878` requires the question to explain the launch
being confirmed. Final retry decisions at `TRIAGE_OUTCOMES.md:17233–17301` do not accept changed-request stale
questions; the cancellation decision concerns queued answers, a different trigger. No exact Planned, BUGS or queue match
was found. Provenance: `launch-retries-implementation-1/f1`, discovery pass 1 in the trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `launch-retries-implementation-1/f1`.
