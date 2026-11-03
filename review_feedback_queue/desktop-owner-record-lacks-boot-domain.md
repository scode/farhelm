# The Linux desktop ownership record lacks its boot identity

Reviewed commit: a98147fb2eaaefb448473107529ada69114d7b03

## TLDR

After reboot, the Linux desktop development build can mistake a manually started supervisor for its old owned process if
their recorded identities coincide, then stop it when replacement is required. An actual collision or loss of user work
was not observed. This argument does not establish a macOS uninstall defect.

## Details

`crates/farhelm-supervisor/src/managed_supervisor.rs:95–106,144–151` persists PID, start token and executable identity,
without a boot discriminator. Its observation at `208–243` compares those saved values with the current process.
`crates/farhelm-supervisor/src/procs.rs:28–35,984–997` explicitly limits the Linux start token to one boot: it is the
raw boot-relative start count. The ownership record can outlive that domain.

An exact cross-boot PID/start/executable alias therefore passes ownership checks. The desktop consumes the match at
`crates/farhelm-ui/src/desktop.rs:995–1005`, selects replacement at `834–847`, and permits escalation at `1025–1033`.
The record was introduced by `4a37e4ea9fe39becefb30e855a41b49cd654316f`; that replacement consumer arrived in
`05ce9d59f38bfcaf5cdc40162c26695c37d7e44b`.

Bind persistent Linux process identity to its boot, or avoid using boot-local identity across boots. The explicit short
check-to-signal PID-reuse acceptance does not cover this persistent alias. Neither does the same-run tooling allowance.
The related future cleanup questions are in Maybe later, not Planned, and concern different mechanisms.

Suggested bucket: `other` for the established Linux development-build scope. Confidence: source-confirmed conditional
identity defect. A qualifying deployment with live user work, the exact collision/replacement sequence, and additional
shutdown/tmux conditions for stronger loss remain unverified after investigation. Evidence establishing that complete
work-loss route would require revisiting the bucket. macOS derives its token from a timeval rather than Linux jiffies;
the Linux argument must not be applied to it. No exact coverage was established.

Provenance: g020/f1 and independent auditor a006/f1; Codex gpt-6.1-sol, high reasoning. The auditor independently
retained the Linux defect and rejected applying the bounded PID-reuse exceptions to it. No collision experiment ran.
