# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`stopped-session-launch-error`](reports/stopped-session-launch-error.report.md) landed 2026-10-16 in #2117: A cached launch error no longer overrides a session's recorded stopped status (gated)
- [`trash-totals-unchecked`](reports/trash-totals-unchecked.report.md) landed 2026-10-16 in #2118: A host's trash totals are marked partial or unknown when unchecked entries exist, instead of showing 0 B (gated)
- [`checkout-override-error-wording`](reports/checkout-override-error-wording.report.md) landed 2026-10-16 in #2119: A checkout failure caused by a per-host folder override names that override instead of pointing at Settings (gated)
- [`rust-test-portability`](reports/rust-test-portability.report.md) landed 2026-10-16 in #2093, #2094, #2097, #2099, #2101, #2103: Rust tests stop failing on macOS temp paths and APFS names, wait for real completion, keep Git config out of fixtures, and the test-state sweep cannot hang (6 outcomes, each gated)
- [`wrapped-link-warning`](reports/wrapped-link-warning.report.md) landed 2026-10-16 in #2156: Honest web links that agents wrap onto another line stop triggering the link mismatch warning, by reading the terminal library's per-cell link tag
- [`browser-test-cleanup`](reports/browser-test-cleanup.report.md) landed 2026-10-16 in #2138, #2139, #2140, #2141, #2142, #2143, #2144, #2145, #2146, #2148, #2149, #2150, #2151, #2152, #2153, #2154, #2155, #2157, #2158, #2159: Browser tests clean up their sessions, templates, preferences and fixtures even when an earlier step fails (20 triage outcomes, each gated)
- [`launcher-fixes`](reports/launcher-fixes.report.md) landed 2026-10-16 in #2160, #2161, #2162, #2163, #2164: New-session dialog fixes: retry key after a template resets permissions, Launch after a repository pick, the setup field after the folder is cleared, custom flags kept on a model choice, and the active repository suggestion kept visible (5 outcomes, each gated)
