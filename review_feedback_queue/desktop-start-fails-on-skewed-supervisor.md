# The desktop app will not start next to an older running supervisor

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If a supervisor the user started by hand is still running from an older Farhelm version, the desktop app fails to start
after 30 seconds with "managed local supervisor did not connect", never mentioning the version mismatch or what to do
about it.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F31 / COR-DESKTOP-SKEW-START`, tagged **definite**. Anchor and title: `crates/farhelm-ui/src/desktop.rs:878` —
the desktop app refuses to start next to an already-running supervisor on another protocol version, with a misleading
message.

When the desktop app launches, it first checks whether a supervisor is already serving its state directory
(`discover_local_supervisor`). A supervisor that answers on a different protocol version is deliberately counted as
"answering" (`crates/farhelm-helm/src/provisioning/backend.rs:106`): starting a rival supervisor over the same socket
and state would be worse, and the helm already has a per-host "version skew" state to surface the mismatch. In that case
the desktop does not spawn its own supervisor (`desktop.rs:370-373`) and reuses the existing one.

The app then runs a readiness gate before opening its window (`await_local_supervisor_until`, `desktop.rs:845-930`). It
polls the embedded helm's host list and proceeds only once the "this machine" host row is Connected. A version-skewed
supervisor never becomes Connected, and neither does one stuck in identity-mismatch or identity-unverified. After 30
seconds startup fails with "managed local supervisor did not connect within 30 seconds" (`:878-884`), shown as a native
alert on macOS. That message is wrong twice: the supervisor is not one the app manages, and the real cause (version
skew) is never mentioned. The window that would show the host's skew state and its remediation never opens.

The realistic trigger is a hand-started `farhelm supervisor run` (which SPEC.md allows, notably on macOS) left running
across an app upgrade. Every launch then fails until the user works out on their own that the old supervisor has to be
stopped. Suggested change: stop polling once the local row reaches one of these frozen states (VersionSkew,
IdentityMismatch, IdentityUnverified). Then either fail fast with that state's details and remediation, saying the
supervisor is not app-managed, or open the window and let the host row show the state. Also drop "managed" from the
timeout text when the app spawned nothing.
