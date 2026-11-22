# The launcher can hide a known container host refusal

Reviewed commit: e114a47f442adf748bbf8848fe95eb70596f5a9d

## TLDR

While Farhelm runs the first Podman check, the launcher can show a generic check-needed message even when the helm
already knows why the host is refused.

## Details

`crates/gui/farhelm-gui/src/launcher/request.rs:169` uses the reported reason only after there is a check timestamp. The
helm can publish an ordinary installation refusal independently of that completion. The launcher masks the reason during
the interval.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`. Findings:
`container-identity-probe/p9, container-identity-store/f9, and container-identity-admission/f14`.
