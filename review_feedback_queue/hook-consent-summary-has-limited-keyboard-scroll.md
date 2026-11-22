# Launcher hook consent has limited keyboard scrolling

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Keyboard focus reveals the hook setup buttons without providing a separate way to scroll the notices. Whether this can
hide the hook’s lasting effects in a supported window remains unverified.

## Details

**other.** Investigated uncertainty.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/hook_setup.rs:98-104;
launcher/view/hook_question.rs:142-173; launcher/view.rs:2591-2622,2729-2732; components/dialog.rs:610-643;
components/tests/dialog.rs:593-627; focus.rs:1207-1253,1332-1338; prompt.rs:798-826,985-1002,1254-1274; bootstrap.rs:58;
SPEC.md:3092-3096,3738-3747; SPEC_impl.md:1221-1235

The launcher caps its notices area and scrolls the focused answer into view. The area has no separate keyboard focus
stop or page-key scrolling from those answers. The enable button’s help mentions all hosts. A separate summary explains
the lasting settings change and that hooks also affect sessions outside Farhelm. The frame-based presentation guard
checks the frame without checking the summary’s visible bounds. The existing overflow fixture uses different text at a
height below the supported minimum. The hook fixture checks for the summary node at a larger size. Source inspection did
not establish a supported layout where the summary is unreachable by keyboard or assistive traversal while Enable
remains usable. The established limitation concerns keyboard scrolling.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-launcher/HL-11`.
