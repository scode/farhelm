# The clipboard endpoint blocks an async worker on native clipboard I/O

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In the desktop app with a slow clipboard backend, copying (or an agent spamming OSC 52) can freeze terminals and the
session list.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F9 / COR-CLIPBOARD-BLOCK`, tagged **possible**. Anchors and title: `clipboard.rs:83-104`,
`crates/farhelm-ui/src/desktop.rs:2376-2396`, `lib.rs:455-464` — The clipboard endpoint runs a blocking native clipboard
write on an async worker under a global mutex

The desktop webview has no working browser clipboard API, so its copy paths POST the text to the embedded helm's
`/api/clipboard`. There, `post_clipboard` (`clipboard.rs:83–104`) calls a native clipboard writer (`ClipboardSink`) that
the desktop shell registered. That call runs directly on the tokio async worker thread handling the request. Nothing
wraps it in `spawn_blocking`.

The desktop's writer (`native_clipboard_sink`, `crates/farhelm-ui/src/desktop.rs:2376–2396`) does all of the following
synchronously. It takes a `std::sync::Mutex` around one shared `arboard::Clipboard`. On first use it creates that
clipboard, which connects to the display server. Then it calls `set_text`. The `ClipboardSink` contract
(`lib.rs:455–464`) says only that the writer must be callable from any worker thread, not that it must return quickly.
Concurrent clipboard requests each park another async worker on that blocking mutex.

Clipboard writes come from terminal text selection and from programs' OSC 52 escape sequences (a way for a terminal
program to set the clipboard), so they arrive in bursts that follow terminal activity. A worker blocked inside a
clipboard call cannot run other tasks, including terminal I/O, the event feed, and other requests. The runtime has only
a few workers.

The premise is that `arboard`'s `set_text`, or the first `Clipboard::new`, blocks noticeably on some platform (plausible
on X11/Wayland, which need round trips to the display server; macOS is normally fast). This was not measured.

Suggested fix: run the writer inside `spawn_blocking`, optionally with a timeout that logs and drops the write, which
fits the endpoint's best-effort clipboard contract. Alternatively, document that writers must return promptly and move
the desktop writer's blocking work to a dedicated thread.
