# Proxy probe failure handling remains partly unverified

Reviewed commit: 7ceb053a0c34908b8f081ec48d2f31fd1b711a1a

## TLDR

The inspected paths refuse a failed proxy read. Whether every failed read stays distinct from an empty result, including
redaction of the refusal, remains unverified.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`crates/farhelm-helm/src/environment/podman_probe.rs:734`.

Reviewed commit 7ceb053a0c34908b8f081ec48d2f31fd1b711a1a. Creation now reads the place manager after the readiness
check. Check whether the new route distinguishes a clean empty environment from read failure and whether failure
diagnostics preserve the proxy privacy contract. SPEC_impl.md:6185–6189 requires the fresh observation. Confidence is
pending the creation_proxy and command-route reads. Possible coverage none.

Latest source check. Reviewed commit 7ceb053a0c34908b8f081ec48d2f31fd1b711a1a. Proposed drop refuted by source.
podman_probe.rs:734–772 falls back to an exact text read when structured output is unavailable, propagates failure or
unreadable settings, and returns an empty map only when every selected key is absent. Lines 617–639 replace raw failures
with a constant reason except for typed SSH session-limit refusals. The production text adapter at :129–135 discards
nonzero stderr and invalid text content. podman.rs:1339–1345 propagates the read refusal. The behavior satisfies
SPEC_impl.md:6185–6189. Could be highest only if a separate route exposes credentials, which these lines do not
establish. Possible coverage none.

The independent check did not settle the proposed exclusion. AMBIGUOUS via 7ceb053a:podman_probe.rs:732–761,
podman.rs:1337–1347, SPEC_impl.md:6185–6189. The text fallback propagates command and parse failures, and creation
propagates a fresh-read refusal before writing settings. The proxy-selection loop, command error handling, and
production adapter are missing, leaving the empty-map condition and diagnostic privacy unverified.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `podman/l1`.
