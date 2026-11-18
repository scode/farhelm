# WebSocket trace logs can expose the desktop helm credential

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

When WebSocket trace logging is enabled, Farhelm’s logs can contain a credential that grants access to the running app’s
terminal, command, and file-transfer APIs.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R4 Sol WebSocket credential in trace (C).

Source references: `crates/farhelm-helm-client/src/ws.rs:81–89`, `crates/farhelm-desktop/src/lib.rs:425–441`,
`Cargo.lock:8121`.

The client puts the live credential in Sec-WebSocket-Protocol for both UI and terminal WebSockets. The pinned
tungstenite 0.29.0 dependency prints the complete request, including the header and address, at
src/handshake/client.rs:188–197. The desktop logging subscriber honors its environment filter and writes to stderr, with
the log bridge enabled.

Sol rated this source-confirmed and critical. Opus verification C confirmed the leak, suggested the security bucket, and
rated it noncritical for the migration: trace is an explicit opt-in, default levels omit the record, and the per-launch
credential is usable only while that app lives. Highest reflects established credential disclosure when logs are shared
or collected.

Cap the dependency's raw handshake trace logging or keep the credential out of the unredacted trace. Verify with a
synthetic credential that tracing ordinary UI and terminal connects never emits it.
