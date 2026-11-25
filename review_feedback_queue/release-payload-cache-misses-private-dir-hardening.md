# Release payload cache lacks the 0700 + symlink-refusal hardening the directory cache received

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

No user-facing impact today. The cache holding the downloaded host binary is created with whatever mode the ambient
umask gives and without refusing a symlinked cache directory, while the operator-directory cache was explicitly hardened
against exactly that. It is protected today only because the surrounding helm state directory is 0700.

## Details

**other.** Source inspection; commissioner confirmed both creation paths. Hardening consistency; no established security
consequence while the state-dir boundary holds. What would move it to highest: the cache being created outside the 0700
state dir, or the state-dir mode regressing, reopening other-account substitution → code execution as the SSH user on a
provisioned host.

`crates/farhelm-helm/src/provisioning/release_payloads.rs:822` (path() miss) and `:1364` (publish_controls) call
`std::fs::create_dir_all` with no mode restriction, no symlink/non-directory refusal, and no mode repair; the write path
also follows a symlinked generation dir, whereas `housekeeping` refuses to follow one. Contrast
`crates/farhelm-helm/src/provisioning/payloads.rs:455-525` (`ensure_private_extracted_dir`: `DirBuilder` mode 0o700 at
creation — "0700 from the moment it exists, independent of umask, so there is never a window where the directory is
briefly world-accessible" — symlink/non-dir refusal, and `set_permissions` 0o700 repair), the deliberate hardening for
the same class of materialized-binary cache.

The reviewer proposed dropping this as no-established-consequence (the helm state dir is created 0700 by
`ensure_private_dir`, supervisor lib.rs:120-127, called at helm lib.rs:1417 and ownership.rs:74 before any download;
same-account writers trusted). Commissioner REJECTED the drop and records it: the asymmetry with a deliberate,
documented hardening of the sibling cache is a real consistency gap worth keeping; the 0700 state dir is a single
protective layer whose regression would reopen substitution; and the fix is small (mirror/reuse
`ensure_private_extracted_dir`). The queue keeps hardening findings of this kind (cf. the UI-channel backlog item).

Verify: read both creation paths. Fix shape: create cache_root/generation dir 0o700 at creation, refuse symlink/
non-dir, repair mode — mirror/reuse `ensure_private_extracted_dir`.

Reviewer metadata (area 5 chunk E, payload staging): suggested bucket: other; confidence: confirmed (gap and 0700
boundary both verified); proposed_drop: yes (no established consequence) — REJECTED by commissioner, recorded;
possible_cover: the F6 directory-cache hardening addresses the sibling cache, not this one; no queue item covers the
release cache (searched). Coverage searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
