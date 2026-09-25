# Window geometry is untracked if restore-maximize is ignored

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On some Linux window managers, resizing or moving the desktop window is never remembered and every launch tries to
maximize again.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F25 / COR-WINDOW-MAXIMIZE-FENCE`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/desktop.rs:557` — If
the window manager never reports the restored maximize, window geometry is not tracked for the whole run

The desktop app remembers the window's position, size and maximized state across launches (`WindowTracker`). If the
saved state says `maximized: true`, `attach` requests maximize and sets a fence, `restore_maximized = Some(true)` (line
557). While the fence is set, every `Moved` and `Resized` event is ignored, so the transient geometry of the maximize
animation is not saved as the ordinary frame. The fence clears only when an event observes
`window.is_maximized() == true` (`restore_maximized_after_event`).

If the window manager ignores or refuses the maximize request, which some Linux window managers do, the window never
reports maximized and the fence never clears. For the whole run, moves and resizes are ignored, and at close `flush`
takes `maximized` from the still-set fence and saves `maximized: true` with the old rectangle. The next launch tries to
maximize again, and so on. SPEC "Desktop window chrome" says the app remembers the last ordinary window rectangle. The
saved-`false` case is unaffected, because the fence clears on the first event. The suggested fix is to clear the fence
after the first settled event even when the observed state disagrees, and to save the maximize state actually observed.
