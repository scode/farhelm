# The shell PATH probe can write outside its private result file

Reviewed commit: 23ae2b9850c539d6d90284dd48322a98af7d78de

## TLDR

The desktop’s login PATH check can write to the wrong file when its configured temporary directory lacks a private
boundary.

## Details

**highest**. Confirmed by source and contract inspection.

The lookup retains the initially opened result file for reading, but the write uses a separate pathname lookup.
Owner-only file permissions protect the file's content. They cannot stabilize its directory entry when another Unix
account can modify the directory. The consequence is a possible write to a different file under the desktop user's
permissions. The route requires a configured temporary directory lacking a private boundary. Neither a private directory
nor a sticky directory that protects the entry supplies that prerequisite.

The relevant source is `crates/farhelm-desktop/src/login_path.rs:32–44` and `:59–68`, with the result-file contract at
`SPEC_impl.md:7486–7491` and the account boundary in `SPEC.md`, “Local authority and trust between hosts.” Bind both
ends to the same file identity or enforce a private directory around the result file. This assessment is based on
source.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `desktop-path/f16`.
