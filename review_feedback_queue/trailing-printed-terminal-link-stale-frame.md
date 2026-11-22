# Printed terminal links can open a different address

Reviewed commit: 0592c5c6d667abe2f08706185b7566febb3434f7

## TLDR

A click on a printed link can open an address from a newer terminal screen than the one the user clicked.

## Details

**highest.** Source inspection. Main reference `crates/gui/farhelm-gui/src/terminal/attached.rs:2695`.

terminal.rs 469–489 and 780–786 update the viewport and cached hover before notifying. Streamed output likewise updates
them at 376–398, 536–558. attached.rs 2365–2377, 2695–2700 and 3982–3997 binds the press to the freshly resolved hovered
link. Release requires equality with the live lookup and opens that captured target (2730–2745, 2781–2810). It prevents
a changed target after the press, but a draw after the press does not retire a target already captured from the newer
model. An ordinary printed address has no rendered disclosure witness (3219–3225). SPEC.md tip 3518–3521 permits that
exemption because the shown address is the one that opens. The renderer explicitly separates the live model, drawn frame
and presentation: at the tip, vendored GPUI window.rs 3711–3714 and 3797–3805 describe drawing/swapping the frame,
3892–3907 submits it to the platform, and 6429–6463 dispatches input using its last drawn hitboxes and listeners. On
Wayland, the window schedules frame work independently (vendor/gpui-pre-linux/src/linux/wayland/window.rs 1021–1058) of
native button callbacks (client.rs 2612–2638. Window.rs 1577–1588). No pre-input draw or destination witness is required
by this path. That supports the wrong-frame press authorization in the shipped path, but it does not establish that the
browser-open callback itself runs before the compositor presents the newer scene. No platform timing, click or browser
result was observed. Attribution: the live binding was already present in parent
77b66797b218a8dae046f1d9d3da76386d6b5a60 attached.rs 2697–2702. The new exit gate and viewport rules (lib.rs 470–475,
viewport.rs 61–74, attached.rs 1477–1535 and 3712–3717) make primary-history links on a retained alternate frame
reachable after exit and allow local scrolling despite retained reporting. The exit state itself leaves a following view
showing the final screen (terminal.rs 789–794. Attached/tests.rs 3021–3042). Newly exposed history enters the existing
admission path when the view is scrolled. Keep the release revalidation, but bind the press to a target from the drawn
frame. Bucket highest for the pre-existing, source-supported violation of the remote-output-to-local-browser
authorization invariant. Do not claim a demonstrated navigation-before-presentation outcome specific to the exit patch.
Confidence confirmed for the handler and dispatch model. Actual presentation timing unverified. Possible coverage none.
Coverage: attached/tests.rs 4154–4191 checks a static printed address and jitter/drag, 5098–5134 settles the
printed-address update before the next click, and 4924–4947 changes an OSC label after mouse down. None tests a live
printed target differing from the clicked frame at admission. No later in-range terminal source or assertion changed
this path. 93b0cd7386d134c3b480bb96f0d7efb9aec42820 changes GPUI window construction for macOS only. The Wayland mouse
and frame paths are unchanged at the pinned tip.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l3`.
