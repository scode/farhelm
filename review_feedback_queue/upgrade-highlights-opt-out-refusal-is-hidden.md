# Don't show again closes silently when the helm refuses to save it

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A refused Don’t show again save can close the highlights dialog without telling the user.

## Details

The visible Don't show again button requests an explicit persistent opt-out (SPEC.md:844–846). It sets the dialog's
opt-out flag and dismisses through its hook (crates/gui/farhelm-gui/src/whats_new.rs:133–143,178–188,344–346). The hook
immediately closes the dialog and sends both the running baseline and the opt-out through a keyless PUT. On a refusal,
its only callback is a warning in the log (crates/gui/farhelm-gui/src/whats_new.rs:87–113). This command path has no
built-in failure display or stored refusal (crates/gui/farhelm-gui/src/operations.rs:603–611,628–668). A valid patch can
still fail while committing the preference row. Both fields are written in one SQL operation whose write and commit
errors propagate (crates/farhelm-helm/src/store.rs:6301–6308). The handler returns an HTTP error for either refusal
(crates/farhelm-helm/src/preferences.rs:319–334). The older release remains eligible if neither field is saved
(crates/farhelm-desktop/src/whats_new.rs:57–76). This applies while the UI connection stays up, so the connection-lost
notice does not cover it (SPEC.md:5452–5456).

SPEC.md:4694–4710 and 5495–5498 require a visible failure unless the write has a best-effort exception. Welcome,
Announcements and some other convenience writes have explicit exceptions. The upgrade-highlights clause and
implementation contract add no exception for the explicit opt-out (SPEC.md:844–846 and SPEC_impl.md:4949–4955). Carry
the opt-out's refusal back to a visible notification or hold its dialog while saving and report the refusal there. The
failure-reporting change can be confined to the user's explicit opt-out. No later commit in the reviewed range changes
the callback.

Confidence: confirmed by source inspection, conditional on a preference-write refusal while connected.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `upgradeUI/l11`.
