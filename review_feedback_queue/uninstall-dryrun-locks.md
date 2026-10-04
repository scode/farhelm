# Uninstall preview omits currently held lock blockers

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Uninstall preview omits currently held lock blockers.

## Details

`F35 / COR-UNINSTALL-DRYRUN-LOCKS` — **definite** — `crates/farhelm/src/uninstall.rs:157` — Uninstall preview omits
currently held lock blockers

`farhelm uninstall --dry-run` prints its removal plan and returns before checking the locks that can block actual
removal. On macOS these include the installer's lock beside the app and the supervisor's and helm's runtime locks. An
open desktop app or an occupied installer lock therefore does not appear as a current blocker in the preview, although a
confirmed uninstall immediately refuses when it tries to acquire those locks. The general preview warning that an
install can cause refusal does not report whether that blocker exists now.

`SPEC.md:311–312` requires dry-run to report proposed actions and blockers without changing files. Inspect existing
blockers without creating directories or lock files, then still acquire the locks and recheck the plan after
confirmation. Preview observations cannot replace those later checks because a process can start or stop in between.

Suggested bucket: other

Possible cover: none identified.

Caveats: Actual removal remains protected by lock acquisition and rechecking. This is a preview-reporting defect
established by source ordering; no runtime check was performed.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `cli_lifecycle p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Actual removal safe; no runtime check.

## Filed reviewer metadata

- `cli_lifecycle p1`: confidence as filed: definite / confirmed. Suggested bucket as filed: `other`.
