# Missing system certificates prevent the entire helm from starting

Reviewed commit: 667b4821b933bc61c7bb348df7a9eabf5a2c634e

## TLDR

On Linux without installed CA certificates, Farhelm’s web UI cannot start, even when no downloads are needed.

## Details

Suggested bucket: `high`. Confidence: confirmed. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop:
none.

The dependency change at Cargo.toml:184 makes client construction fail when Linux has no usable CA roots. Contrary to
Cargo.toml:163–167, this failure is fatal to helm startup: provisioning/payloads.rs:1121–1133 unconditionally evaluates
release_client()? before selecting a payload source; service.rs:308, lib.rs:571–577 and lib.rs:1977–1986 propagate the
error out of startup. This also blocks --payload-dir and developer builds, whose non-downloading branches are at
payloads.rs:1202–1213. Trigger: run a helm on a minimal Linux installation without a usable CA bundle. Consequence: the
entire helm exits rather than starting with unavailable downloads and feedback. Existing sessions are not shown to be
killed. Construct the download client lazily or contain its failure in the download source, and avoid constructing it
for local payload sources. Confirmed by source inspection, without reproduction.

Possible cover: Cargo.toml:160–171 and releasing/changelog.d/os-certificate-store.md:5 explicitly accept unavailable
HTTPS without CA certificates, but do not accept unavailable helm startup; the comment expressly says the helm survives.
TRIAGE_OUTCOMES.md heading embedded-payload-cleanup-blocks-helm-start.md concerns a different trigger. No matching
Planned item, BUGS entry, queue item or filter found.

Current-main status: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. Cargo.toml,
provisioning/payloads.rs and provisioning/service.rs are unchanged; lib.rs:578–584 and 2004–2013 still propagate the
startup error. No fixing commit found.

Review coverage: the full commit diff and relevant failure paths were read. No product runtime tests were run; unchanged
surrounding code was inspected only where relevant.
