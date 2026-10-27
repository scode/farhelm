# File-open symlink regression also fails the regular-file check

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

The installer's symlink test would still pass if its no-follow protection disappeared, because the fixture points to a
directory that another check rejects.

## Details

`crates/farhelm-supervisor/src/installation/dirs.rs:310–319` supplies a directory as the symlink target. Without
`NOFOLLOW`, opening succeeds as a directory and the separate regular-file requirement still refuses it. A symlink to an
otherwise admissible regular file would distinguish the intended no-follow guard.

Suggested bucket: `other`. Confidence: confirmed fixture/oracle gap; the production guard is present and no production
redirection defect is established. Possible cover: none exact found. Provenance: `g009/f1`, corroborated by `a003`. No
tests or filesystem experiment ran.
