# Restart with trusts replacement folders through stale membership

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

## TLDR

With automatic workspace trust enabled, Restart with can trust a replacement folder because a resumable Codex or Pi
session still belongs to the old managed checkout. Borrowing sessions skip the origin's folder-identity checks; no
vendor execution was attempted.

## Details

Suggested bucket: **highest**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-helm/src/sessions.rs:2833-2851`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Later-turn synthesis at reviewed commit 463cac1528b5c5e259404975380d321a3fff0412; same underlying defect as producer
g112/f1, originally prose lead 1. Correction: affected automatic per-launch Restart with harnesses are Codex and Pi
only. Claude is excluded by launches.rs:495-508; Muse has no composed resume command (launches.rs:615-623), and
supervisor core.rs:3277-3282 refuses Restart with without one, including named Restart's existing resume-template
precondition. Earlier Muse inclusion is retracted under this same ID. The borrower route is separately established:
working_copies.rs:1148-1180 attaches ordinary sessions to allocated ancestor rows by canonical path; :960-973 finds
origins only by origin_session_id, so a borrower has no origin row. Restart core.rs:11071-11094 only checks member
allocation state; origin-only recovery/Ready validation at :11099-11110 is skipped for that borrower. Its later cwd
check :11247-11255 uses canonical path spelling (prior read :3143-3163), not inode or birth time. Helm still grants
trust from the retained membership at sessions.rs:2833-2851. Thus a replacement already present at the same canonical
path before the request can receive trust: a borrower does not require inode reuse, unavailable birth time, or a
check-then-swap race. Assumptions: retained allocated membership; borrower is a resumable Codex/Pi agent launch; enabled
corresponding trust setting; accepted restart offer/selector and ordinary runtime prerequisites. Origin route remains
narrower: Ready preparation and matching root/device/inode with absent birth time are needed for the original preserved
scenario. Suggested highest for the source-established trust-boundary consequence, conditional on those operation
premises; runtime occurrence not measured, no vendor execution claimed. DISAGREE with exclusion for lack of source
route, ordinary new-session repair, or deliberate post-verification swap acceptance. possible_cover: broad ledger
no-birth-time fix remains ambiguous duplicate-decision coverage for the origin subcase, but does not cover
birth-time-independent borrower trust; deliberate same-account post-verification replacement covers a distinct trigger
only. proposed_drop:false. Parent must retain the borrower scope and unresolved coverage distinctions; do not duplicate
it as a new finding.

Provenance: `a034/g112-restart-trust`, `g112/f1`, `g139/f2`, `g188/f1`, `g207/f2`, `g208/f1`. These are reviewer-local
identities, not additional findings. Proposed drop: none for the retained remainder.
