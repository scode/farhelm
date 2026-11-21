# No end-to-end test starts a first-class Bash or Zsh session

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

No end-to-end test starts a first-class Bash or Zsh session through its normal launch path.

## Details

**other**. Confirmed by source and contract inspection.

the new shell choices can regress between the launcher and the interactive terminal without a test catching it.
SPEC.md:5568–5569 promises first-class Bash and Zsh sessions with the generic activity reader. SPEC_impl.md:5183–5186
promises PATH resolution and the empty Generic runtime argument slot. Structured_launches.rs:89–95 explicitly excludes
the shells from the executable fixture. The compilation assertions (crates/farhelm-helm/src/launches.rs:1472–1503) stop
before launch, and the new agent-spawn test (crates/farhelm-helm/src/agent_requests.rs:17244–17336) uses a duplex
supervisor peer and asserts receipt of the structured command rather than the resulting terminal. The pinned test
sources contain no structured-shell runtime case. The existing plain-shell reboot test
(crates/farhelm/tests/e2e/boot_id_durable_outcome.rs:321–358) checks shared interruption and resume messaging. No
subsequent runtime test in the reviewed range. The later shell tab test is presentation coverage. A source-level
coverage conclusion only. Confidence confirmed. Bucket other (test gap, no runtime failure inferred). Possible_cover:
existing generic terminal and reboot tests cover part of the shared runtime. Turn-2 synthesis: shell-contract/f6 is the
same structured-shell integration gap, with additional neighboring coverage references for missing bare executables and
wrapper errors. The scope is selection through an interactive terminal and the expected Error/Exited, status and
lifecycle results, as in SPEC.md:1381–1383 and :5568–5569. Shell-permissions/f6 is a separate approval-card display test
gap. It does not exercise terminal creation. Deduplicate the shared-runtime test finding with shell-contract/f6. No
inferred runtime failure or bucket change.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f6`, `shell-runtime/f2`.
