# Automatic updates can resume after a settings read fails

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Farhelm can install an update in the background after you turned automatic updates off, if it later cannot read the
setting.

## Details

Haiku pass H-P1-4, checked by gpt-6.2-sol. Suggested queue bucket **other**. Likely. The fallback is confirmed in
source. A post-startup read failure persisting through the final gate was not observed.

- Verdict: NOT CRITICAL
- Route checked: `crates/farhelm-desktop/src/updater.rs:432-435,470-481,958,1343`, `state.rs:75-105`, `lib.rs:237-239`;
  old app `24bc0580:crates/farhelm-ui/src/desktop/updater.rs:472-483`. The queue's
  automatic-update-opt-out-can-roll-back.md covers remote hosts and SQLite durability, a separate mechanism.
- Reason: SPEC.md:808-813 promises that an opt-out stops background desktop installs. An unreadable file is treated as
  on even at the final install gate, despite an intact opt-out potentially still being on disk. Ordinary writes use
  atomic replacement and the app's initial state load rejects unreadable or malformed state before starting the worker.
  The route needs a state failure after successful startup that persists or recurs through the install gate, and the
  installer still requires a verified newer release. No ordinary ordering that overrides an intact opt-out was
  established. The fallback predates GPUI.

Keep the last successfully read setting for background work and surface or log the read failure. Add a fake state-read
failure after a successful read of an explicit opt-out and check that the final install gate refuses the background
install.

Possible cover: SPEC.md, “Healthy local filesystems,” accepts failure caused by broken storage. Its scope for an install
proceeding under a different preference is unclear. SPEC_impl.md:7417–7419 defaults an absent setting to on.
`automatic-update-opt-out-can-roll-back.md` concerns remote-host SQLite durability.
