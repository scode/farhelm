# Claude trust Settings omits its home and root exceptions

Reviewed commit: 6f9918a88100c8d8a69ae471fe12cfde4c7bf76d

## TLDR

The Claude trust setting says it applies to folders the user chooses. Its help omits that Farhelm skips the home
directory and filesystem root.

## Details

`crates/gui/farhelm-gui/src/settings/agent_trust.rs:203` describes the selected-folder mechanism without those
exceptions. The product contract and supervisor enforce both exclusions, so a user reading and enabling the switch
receives a broader description than the launch can honor.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`. Findings:
`agent-trust-ui/f1 and claude-trust-application/f10`.
