# Published uploads lack directory fsync

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A crash could lose an attachment that was already acknowledged.

## Details

F89 — **possible** — `crates/farhelm-supervisor/src/files.rs:636` — Published uploads lack directory fsync

Upload publication synchronizes file contents but does not synchronize the directory entry naming the file. On a
filesystem where that directory update can disappear after power loss, the acknowledged attachment could be lost while
its session record survives. No filesystem-specific crash reproduction establishes observed loss. Settle the promised
durability contract for acknowledged attachments; if it requires this persistence, synchronize the containing directory
as part of publication before acknowledging success.

## Evidence and triage context

- files.rs:649 syncs the file before :658 creates the published hard link; :666-667 removes staging and returns without
  synchronizing the publication directory. uploads.rs:1209-1214 runs that publisher and :1314-1319 sends UploadCommitted
  after success.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:2206-2217 describes complete attachment publication and retention until session deletion.
  files.rs:134-137 discusses a crash mid-upload which the client sees as failure; that does not cover a response already
  acknowledged before machine failure. No exact Planned, BUGS, queue or ledger acceptance found.

Caveats:

- No filesystem-specific crash reproduction. Keep as a durability contract question, not a confirmed observed loss. The
  hook-report reboot-anchor refutation does not apply to retained attachment files.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ss_data:p1:C5`.

- `ss_data:p1:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
