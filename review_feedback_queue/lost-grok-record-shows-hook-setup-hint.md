# Losing a Grok conversation record can bring back setup advice after the hook already worked

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

Losing a saved Grok conversation can bring back setup advice even though Farhelm already received a successful hook
report.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/session_pane/hook_band.rs:23-33,193-198;
crates/gui/farhelm-gui/src/session_list/restart.rs:104-107,134-140; crates/farhelm-proto/src/lib.rs:959-975

After a manual Grok hook has enabled Restart, losing the exact saved conversation record can bring back the setup hint
and tell the user to enable automatic hooks and start a new session. The old hook already reported the conversation. At
bb31959c2f22c96bbbb72da2e60eee4a0b288a6b, a launch with auto-setup off records NotRequested
(crates/farhelm-supervisor/src/service/core.rs:14755-14759); a hook already on disk can still report, as pinned
SPEC.md:3969-3972 states. On an exact record absence, verify_grok_resume keeps the locator with resumable=false, writes
it, posts ResumeWithdrawn, and refuses Restart without relaunching the agent
(crates/farhelm-supervisor/src/service/core/vendor/grok.rs:18-65; pinned SPEC.md:4019-4025). IntegrationSnapshot maps
that locator to NotCaptured (agent_kind/mod.rs:1928-1938; service/status.rs:301-317,339-349). The existing source check
at service/core.rs:22158-22181 documents the same public offer transition and preserved identity. The new GUI treats
NotCaptured as never having a usable report: the pane offers setup again and says it is waiting to learn the
conversation, and the Restart tooltip sends the user to enable setup. The independent withdrawal notice explains the
actual record loss. Pinned SPEC.md:3098-3101 scopes the hint to waiting for capture, and :3367-3374 requires the
specific Restart blocker. Preserve the current withdraw-versus-never-captured reason in the pane and tooltip, or show
wording that handles both when the reason is unavailable. This affects one running launch with no dismissal needed, and
does not cause the record loss or relax Restart. The reviewer suggested other; confirmed by source. Pinned tip
1a53d9d35889d7dde96e31320cc1681dd013737d retains the same decision route. The live route is explicit:
service/core.rs:12473-12499 performs exact verification before the live-terminal probe and before any stop, and
service/capture.rs:102-117,300-339 rereads and publishes the changed hook capture for that generation. No GUI reconnect
or later launch is involved.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-session/HS-13`.
