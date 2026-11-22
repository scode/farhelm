# Launcher comparison waits use the accessibility label as painted text

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Launcher comparison scenes wait for mixed-case heading text while the launcher paints it in uppercase.

## Details

**other.** Source inspection.

docs/gpui/screenshot-scenes.json:1505–1517,1528–1549,1621–1649;
crates/gui/farhelm-gui/src/launcher/view.rs:1647–1654,2431–2435;
vendor/gpui-agent/crates/cli/src/main.rs:1246–1249,1270–1280 at 1a53d9d35889d7dde96e31320cc1681dd013737d

The launcher comparison batch can stop immediately after opening New. It waits for mixed-case Destination and uses the
same text to observe dismissal. The heading changed from mixed-case painted text in launcher/view.rs:2380 at
42de88aa6135204857890791d078352985076fb1 to uppercase at launcher/view.rs:2384 in
82c5c23129a913833d0a343026870ccbbc9579e5. At the pinned tip the section renderer still paints uppercase verbatim.
Destination is only its accessibility name. gpui-agent wait --text and --gone match visible painted text runs
case-sensitively, so the heading cannot satisfy the open guard or keep the dismissal predicate false. A separate
occurrence would pass the wait for the wrong reason. The case mismatch also masks the older Runs on steps flagged in
LV-13. Retarget readiness to a stable rendered id and use a matching dismissal condition for the same surface. This
issue is independent of layout and does not establish the post-scroll geometry in LV-10. The reviewer suggested other
(maintainer screenshot comparison). Confidence: confirmed for the incorrect predicates.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-visual/LV-15`.
