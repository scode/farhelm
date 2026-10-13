# A blocked socket probe can hang later test runs

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

An abandoned terminal server can make subsequent test runs hang before their fixtures start, despite the cleanup time
limits.

## Details

Suggested bucket: `other`. Confidence: confirmed blocking mechanism; the trigger was not reproduced against a real
wedged tmux server. Source: adversarial tooling/release reviewer. Proposed drop: none.

Added by `dc0cfdd5b20f1deadb5333b5e17416cb295900c6`. `crates/farhelm-teststate/src/lib.rs:510` calls blocking
`UnixStream::connect` before the five-second child deadline starts at `530`. A listening Unix socket with a full accept
backlog can block this call indefinitely. The sweep's thirty-second budget (`220–228`) cannot interrupt it either.
`tempdir_with_sweep` invokes the sweep synchronously (`672–677`), so a stalled abandoned fixture can prevent unrelated
Rust fixtures and the browser stack's startup sweep from progressing.

The reviewer demonstrated the syscall mechanism with a private Unix listener using `listen(0)`, one pending connection,
and a second blocking connect in an owned child. That child remained blocked for the one-second observation and was then
killed and reaped. No product process or repository test was used. The new unresponsive-listener test (`1080–1117`) does
not fill its backlog, so it does not cover this boundary.

Make the preliminary probe nonblocking or deadline-bound. An inconclusive probe must preserve the directory and let test
startup continue. Possible cover: FILTER.md's "A hung private tmux server or systemd user manager" describes the
affected host's supervisor or requests stalling; this consequence instead reaches subsequent shared harness startup,
outside that named scope.
