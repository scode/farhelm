# An upload can paste its path into a restarted agent

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

If you restart a session during an upload, the file can finish normally while its path is pasted into the new agent,
possibly in a different conversation.

## Details

Bucket: **high**. GUI migration final whole-tree review, R2 Sol upload after Restart (E).

Source references: `crates/gui/farhelm-gui/src/terminal/attachments.rs:622–626`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:1681–1691`, `crates/gui/farhelm-gui/src/session_pane.rs:390–420`,
`crates/farhelm-supervisor/src/service/core.rs:12696`, `crates/farhelm-helm-client/src/terminal.rs:644`.

Drop a file in the agent terminal, then Restart the session without changing the selected session. The old attachment is
detached while the same GUI terminal owner survives. The upload completion carries only name and path.
attachment_uploaded stamps the paste with the current live attachment, so a new process attached before completion
receives the old upload's path.

Sol rated the source route confirmed and critical. Opus verification E confirmed it and rated it noncritical: same
session and host, with a bracketed path paste and no Enter. SPEC.md lines 3245–3246 says to use the cursor at
completion. That may cover the new cursor within one attachment, but whether it covers an intervening restart is
unclear.

Stamp the upload with the drop-time attachment. On mismatch keep the uploaded file and use the existing visible “not
typed” outcome. Verify against Restart and Restart with while an upload is pending.
