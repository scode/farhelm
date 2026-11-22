# Tab resume can pass without receiving flood output

Reviewed commit: 06bc10e8e3038b241c01e298aa07121adee0564f

## TLDR

The terminal-tab regression can pass while the paused tab fails to deliver the output produced during its pause.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/terminal_tabs.rs:1536`.

The test resumes the tab and accepts the prefix FLOOD-. The new comment immediately above the assertion acknowledges
that the shell's echoed command contains the same prefix, so this check can be satisfied without a produced record.
SPEC_impl.md, Terminal host / Grid streams, requires working per-attachment flow control. Keep the agent round trips and
make the tab assert a complete output record distinct from command echo, received after resume. Bucket: other, because
the evidence establishes a test gap. Confidence: confirmed by source. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/f1`.
