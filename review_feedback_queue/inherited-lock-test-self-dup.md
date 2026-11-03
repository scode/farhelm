# The inherited-lock test can omit its lock handoff

Reviewed commit: d5a7f8de5ea949628ce146dcd7944852baa40db1

## TLDR

A terminal-host regression can pass without receiving the inherited version lock it claims to test. This is a
conditional test-fixture defect, not an established failure in Farhelm's lock handling.

## Details

`crates/farhelm/tests/terminal_host_entry.rs:109–128` opens the locked file and uses `dup2(lock_fd, 8)` before exec. If
the opened file already occupies descriptor 8, self-duplication succeeds without clearing `FD_CLOEXEC`. Exec then closes
the descriptor before Farhelm starts. The assertions at `157–175` can pass without testing release of an inherited lock.

Choose a distinct destination or explicitly clear close-on-exec, and establish that the starter actually inherited the
descriptor before releasing its specification. The sibling test in `crates/terminal-host/tests/host_process.rs:484–508`
already handles this collision and checks the premise. The second entry-point regression independently tests bypass of
own-version lock acquisition; this finding does not declare that separate coverage absent.

Suggested bucket: `other`. Confidence: confirmed conditional source defect. Whether an ordinary runner assigns
descriptor 8 was not observed; no reproduction ran. No exact specification, Planned item, unlanded plan, known bug,
queue item, ledger decision or filter covering this fixture gap was found.

Provenance: trailing-review group g015, finding f1; Codex gpt-6.1-sol, high reasoning. Retained fields and
qualifications come from its source checks and later synthesis.
