# Token-rotation test requires a filename APFS refuses

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

The new token-rotation test fails on a normal Mac before it reaches the credential-safety behavior it is meant to test.

## Details

Suggested bucket: `other`. Confidence: likely; macOS execution was not performed. Source: adversarial helm/CLI reviewer,
with parent comparison against the repository's existing APFS fixture correction. Proposed drop: none. Possible cover:
none found for this test.

Added by `07b7906519301d115802a8b388a43a3f37bb6663`. `crates/farhelm-helm/src/token_control.rs:1120–1130` gates
`rotate_refuses_non_utf8_before_changing_state` on Unix and unconditionally creates a directory named `state-\xff`. APFS
refuses invalid UTF-8 names with EILSEQ, so `create_dir(...).unwrap()` fails before the token is set up or rotation is
exercised. This affects macOS development/workspace validation; the current hosted macOS release selections run
uninstall and desktop tests, not this helm library test (`.github/dist-build-setup.yml:325–346`). The same platform
constraint is already handled in `crates/farhelm/src/uninstall/locks.rs:371–375`, corrected during this review range.

Run this exact unit test through the recorder on macOS to confirm. Exercise the non-UTF-8 refusal before filesystem
creation through an appropriate seam or constrain the on-disk byte-name fixture to filesystems that support it. Preserve
evidence that refusal precedes lock creation and credential changes; merely catching fixture creation failure would not
prove that contract.
