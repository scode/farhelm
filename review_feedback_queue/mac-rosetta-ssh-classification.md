# Translated SSH inspection may misclassify Apple-silicon hardware

Reviewed commit: 25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652

## TLDR

Unverified after investigation: a remote Apple-silicon Mac may be refused as an Intel Mac if its SSH inspection runs
under Rosetta. No supported command chain producing that result was established, so this is a conditional classification
concern, not a demonstrated setup failure.

## Details

`crates/farhelm-helm/src/provisioning/backend.rs:783` runs the remote inspection through plain `sh -c`, and its prefix
at line 1632 selects no architecture. The Mac probe at line 3893 uses `uname -m`; its parser at lines 3919–3923 accepts
only `arm64`. By comparison, `scripts/install.sh:707` explicitly handles the Rosetta distinction with
`sysctl hw.optional.arm64` before choosing the native payload. `SPEC.md`, "Topology", supports Apple-silicon SSH hosts
and excludes Intel hardware, while "Supported user environments" does not explicitly exclude translated shells.

Investigation checked the complete remote command wrapper and parser, the installer's hardware fallback, Apple's
[Rosetta documentation](https://developer.apple.com/documentation/Apple-Silicon/about-the-rosetta-translation-environment),
Apple's uname source (`shell_cmds/uname/uname.c:143–145,275–280,341–342`), XNU's native `HW_MACHINE` implementation
(`kern_mib.c:515–533`) and execution paths, and the architecture-preference manual. Those establish translation and
native architecture reporting, not the exact inherited translated context this probe needs.

[OpenSSH's session implementation](https://github.com/openssh/openssh-portable/blob/master/session.c) at lines 1431–1435
and 1561–1569 establishes that a command request invokes the user's configured shell with `-c`.
[Apple's arch manual source](https://github.com/apple-oss-distributions/system_cmds/blob/main/arch/arch.1) at lines
48–104 describes default slice selection and explicit preferences, but does not settle translated-parent inheritance
through the additional plain `sh` and `uname` children.

The missing premise is a supported remote bash/zsh configuration making this exact shipped SSH inspection report
`x86_64` on Apple-silicon hardware. Available source and manuals did not settle it; this source-only review ran no SSH
or Mac experiment. An authorized retained observation of that exact chain, or authoritative documentation establishing
its inheritance behavior, would settle it. If confirmed, distinguish hardware capability from translated process
architecture as the installer already does.

Priority: `other`, preserving the reviewer's label. Confidence: **unverified after investigation**, likely conditional
on the missing execution premise. No security, process-loss or data-loss consequence is established. Proposed drop:
none. Possible cover: no exact accepted translated-shell restriction or other recording coverage found. Typed
target-to-archive tests do not settle hardware classification.

Provenance: trailing review `9e3ab60b86025ca74948ededd09b99cd81a2477c..25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`, original `mac-contract/MC-D2`; later interface copies retain its uncertainty.
