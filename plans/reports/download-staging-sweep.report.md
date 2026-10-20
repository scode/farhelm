# Desktop download cleanup on Quit

## What this was about

Quitting the desktop app during a download could leave hidden partial files in Downloads because normal exit skips the
save object's cleanup. Triage approved a fix within about 100 lines. The first prototype recovered leftovers at startup
using ownership locks, but it could not safely classify older unmarked directories and was blocked without publication.

The maintainer then chose cleanup on normal Quit instead: track this process's active staging folders, remove them
during event-loop teardown, and refuse new saves once quitting. That decision explicitly leaves earlier-build, crash and
force-quit leftovers alone. The draft implements that replacement design in 99 lines of code, tests and implementation
notes, within the gate.

## Things you should know

Cleanup runs before the desktop event loop's hard exit and does not wait for transfers to finish. It removes only
directories registered by this process; it scans no directory and uses no inter-process locks or ownership markers.
Completed saves outside staging survive. A completion already publishing as Quit begins may still leave its completed
file, consistent with the existing publication race. Ordinary failed or cancelled transfers retain their cleanup.

Pinned Dioxus and Tao source confirms that the hook runs before process exit, including the macOS
application-termination callback. This is source evidence plus a desktop compile check, not execution of macOS Quit. The
regression keeps actual partial bytes and their save owner alive while cleanup runs, then checks removal, preservation
of a completed save and a separate owner's staging, and refusal before a new staging directory can be created.

The feedback item was removed, and its triage record now reflects the maintainer's revised criteria and this PR.

## Open questions and possible follow-ups

No decision remains open. Earlier-build, crash and force-quit leftovers remain intentionally uncovered. The normal macOS
Quit path was not exercised on this Linux executor; a future native test could verify that integration directly.

## The PRs

- [#2440: clean partial desktop downloads on quit](https://github.com/scode/farhelm/pull/2440/changes), draft against
  main.

## Checks run, reused and skipped

- Recorded `cargo nextest run -p farhelm-helm --lib -E 'test(downloads::)'`: 17 passed, 1216 selected out, zero failures
  or retries. Run `a5d4f6bf-6c64-488f-a8bb-4c0a3344b57f`, nextest 0.9.148, pinned tmux 3.7c. Source identity and JUnit
  are complete; output reached EOF without truncation and process cleanup completed. No runtime substrate-skip marker
  was present. Coverage includes the new Quit regression and existing save, cancellation, publication and opening tests.
- `cargo clippy --all-targets -- -D warnings` and `cargo check -p farhelm-ui --features desktop` passed.
- Cargo formatting, changed Markdown dprint, changelog-fragment format, instruction-pointer checker and test-delay
  checker passed; the latter inspected 305 delays with no unexplained delay.
- These commands covered base `ab2d2e02` plus the implementation. Their evidence is reused after a careful rebase onto
  `18a544c1`: the full upstream change contains only unrelated review records and their index. The functional diff is
  unchanged. Markdown and instruction checks were rerun for the combined tree; the final ledger PR link cannot affect
  executable coverage.
- The old startup-sweep prototype's results were not reused for this replacement. No full Rust battery, browser,
  website, installer or hosted CI run was warranted: this changes native staging ownership and an exit hook, covered by
  the focused Rust cases and desktop compilation. No actual AppKit runtime result is claimed.

## Review gate outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reported no findings after checking correctness,
design, language idioms, the revised acceptance criteria, the full test-authoring checklist and the 99-line size gate.
The mandatory independent resume check passed, and the commit message passed its fresh cold read. A separate
documentation pass covered every touched code file. No substantial departure from the revised design or repeated
corrective review required another scope checkpoint.
