# GNU stat on a Mac breaks existing-artifact Update

Reviewed commit: 25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652

## TLDR

A remote Mac whose SSH command environment puts Homebrew's GNU tools first on PATH can pass setup inspection but
repeatedly fail Update or setup recovery against existing files. The operator must repair command selection before
retrying. A completely fresh installation with missing destination files is not shown to fail.

## Details

`crates/farhelm-helm/src/provisioning/backend.rs:3886` checks that `stat` exists, without checking its dialect.
`metadata_on_target` at line 1004 selects BSD arguments solely from Darwin's OS name, then invokes bare
`stat -L -f '%Lp' -- <path>`. Existing-file callers propagate failure before upload at line 1325, during installation at
line 1383, and for service-file installation at line 1122. The SSH command builder at line 783 and its runtime prefix at
line 1632 leave PATH intact.

[Homebrew's coreutils guidance](https://formulae.brew.sh/formula/coreutils) documents putting `gnubin` first on PATH to
use unprefixed GNU command names.
[GNU's option parser](https://github.com/coreutils/coreutils/blob/master/src/stat.c#L1837-L1858) makes `-f` select
filesystem status without consuming a format; `%Lp` becomes a file operand. An absent percent-named operand fails, and
even an existing one does not produce the required octal mode. This is a source-derived command mismatch, not a runtime
reproduction.

`SPEC.md`, "Topology" and "Supported user environments", does not exclude this documented stock-tool configuration.
`SPEC_impl.md`, "Provisioning", says Mac artifact inspection uses BSD stat, but inspection does not establish that the
PATH-selected executable is BSD stat. Pin the native utility or validate and consistently use the actual dialect. The
separate progress-size invocation at `backend.rs:1999` has the same dialect mismatch, but it does not establish a
fresh-install failure or an upload stall.

Priority: `high`, for a repeatable failure of normal host Update requiring manual environment repair. The originating
reviewer initially suggested `other`, then agreed with `high` after tracing the existing-artifact callers. Confidence:
confirmed source route for the stated PATH and existing-file conditions; prevalence and runtime occurrence unmeasured.
No security or user-work loss is established. Proposed drop: none. Possible cover: an interpretation of "native path" as
requiring a BSD-only PATH is not explicit; no exact coverage or filter match was found. The earlier triage decision
`uninstall-command-fixture-requires-gnu-stat.md` concerns a Linux script executed by a Mac test, not production Mac
provisioning with GNU stat selected.

Provenance: trailing review `9e3ab60b86025ca74948ededd09b99cd81a2477c..25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`, original `mac-impl-contract/mac-impl-contract-discovery-stat-path`. Planning,
contract and install interface records corroborate the same root. The changed target is the Mac provisioning contract at
`SPEC_impl.md:7506`; implementation context is the Mac provisioning addition in
`c3bee0e586b954f24a12e665a961eb9e96bf50ca`.
