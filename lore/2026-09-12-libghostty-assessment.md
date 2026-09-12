# Could Farhelm use libghostty for its terminal?

NOTE: Historical artifact, written 2026-09-12. An assessment of replacing the xterm.js island with libghostty, either
natively on macOS or through WebAssembly, as things stood on that date. Nothing was built; this records what was found
and the conclusion drawn, so the question does not have to be researched from scratch next time. Not maintained. The
upstream projects named here move fast, so verify every claim before acting on it.

## TLDR

Native libghostty on macOS is a poor fit for how Farhelm is built and would be a large rewrite for one platform. The
WASM route is real, exists today in the form of Coder's `ghostty-web`, and is the only one worth considering. It is
still early, and it would cost a few weeks of adapter work rather than being a drop-in swap. The decision at the time was
to park it as a "maybe later" item in TODO.md.

## What Farhelm's terminal is today

The terminal is xterm.js 6.0.0 vendored byte-identical to upstream, mounted as a JS island inside the Dioxus tree
(SPEC_impl.md, "Terminal widget: xterm.js island"). PTY bytes flow WebSocket to `term.write()` directly, bypassing Dioxus
state. The desktop app is that same web UI inside a wry WebKit webview; there is no native rendering anywhere. Around
the island sits about 4,800 lines of `terminal.js` that carry the parts of the product xterm.js does not provide on its
own: the watermark backpressure protocol to the supervisor, OSC 52 clipboard via `@xterm/addon-clipboard`, shift-Enter
handling, copy-on-select, reconnect replay and prefill, cursor resynthesis after replay, and a throttled `refresh()`
workaround for an observed xterm.js scroll-freeze defect.

The xterm.js calls that code actually depends on, counted from the assets:

- `write` (with completion callback), `onData`, `paste`, `open`, `dispose`, `focus`, `rows`, `cols`, `element`
- `refresh` and `buffer.active` (both only for the scroll-freeze workaround)
- `parser.registerOscHandler` (the OSC 52 path and a promise-returning trick the backpressure code leans on)
- `onBinary` (mouse reports and other non-UTF-8 input ride a separate binary channel)
- `onKey`, `onResize`, `attachCustomKeyEventHandler`, `hasSelection`, `getSelection`, `clearSelection`,
  `scrollToBottom`, `loadAddon`, `options`
- addons: `FitAddon`, `ClipboardAddon`

## Native libghostty on macOS

I read `include/ghostty.h` from upstream (1,278 lines on this date) and the `ghostty-org/ghostling` reference embedder.
Three structural problems, each serious on its own:

libghostty owns the PTY. The surface config has `command` and `working_directory` fields and the surface spawns the
command itself. There is no C API for feeding bytes in; the only "io" in the header is an export-terminal-io action.
Farhelm's terminal content arrives over a WebSocket from a remote supervisor. The workaround would be a shim process
that libghostty spawns as the "shell" and that pumps WebSocket bytes through the pty, with resize forwarded via SIGWINCH
in the other direction. That is a hack under the most important widget in the product.

It renders into an NSView with Metal (`ghostty_platform_macos_s` carries an `nsview`). Farhelm's desktop is a Dioxus
tree inside a WebKit webview. A native terminal would be a subview floated over the webview at whatever rectangle the
DOM says the pane occupies, and every overlay the DOM draws over the terminal (dialogs, the profiles popup, drawers)
becomes a z-order fight with a native view. The sidebar bar and tabs would have to be pixel-synced from JS to Cocoa.

Two terminals would have to be kept in parity. The web build keeps xterm.js regardless, so all of terminal.js's behavior
would be reimplemented in Rust against the C API for desktop only. The Playwright suite, where WebKit stands in for the
desktop engine, cannot see a native view at all, so the desktop terminal would lose its only automated coverage.

On top of that: the full libghostty API is explicitly unstable and used primarily by the macOS app; ghostling needs Zig
0.16 and CMake 3.19+ with Ninja and builds the library as part of its own build; there is no cargo crate. That adds a
Zig toolchain to the release pipeline for a Metal view we would have to fight the webview to place.

Verdict: not viable without redesigning the desktop app around a native terminal, which is a different product decision
than "swap the terminal engine".

## WASM: libghostty-vt and ghostty-web

libghostty-vt is the inner layer: VT parsing, terminal state, and a render-state API (`include/ghostty/vt/render.h`),
with no drawing. On this date upstream publishes `ghostty-vt.wasm` (about 1.0 MB) and `ghostty-vt-small.wasm` (about
750 KB) on the rolling `tip` release, updated the same day I looked, alongside an xcframework and a source tarball. I
could not confirm any versioned Ghostty release carrying the wasm; as far as I can tell it is `tip` only. Farhelm
vendors xterm.js byte-identical to upstream so its provenance is checkable, and a rolling `tip` blob gives nothing
stable to pin against. That is a real, if boring, problem.

A VT core with no renderer is exactly the shape SPEC_impl.md rejected when it turned down a pure-Rust wasm terminal
("a project in itself"). What changed is that Coder built the renderer: `coder/ghostty-web` wraps the wasm in an
xterm.js-compatible API with a canvas renderer, a fit addon, selection, link providers, IME, and a dirty-row 60 fps
render loop. It was created for their Mux desktop app. Mitchell Hashimoto separately reported that the wasm build now
beats xterm.js on IO, reflow, and rendering in his own harnesses, and that upstream provides prebuilt wasm; ghostty-web
says it will consume that official distribution once available but on this date still builds from Ghostty source with
its own patch.

Maturity signals as of 2026-09-12: ghostty-web is at v0.4.0 (released 2025-12-09), about 2,800 GitHub stars, commits
through July 2026 with no newer tag, MIT licensed, about 400 KB bundle. Its changelog mentions iOS support but nothing
about WebKit on macOS, which is the engine Farhelm's desktop actually runs in.

What terminal.js uses versus what ghostty-web's `lib/terminal.ts` implements:

| terminal.js dependency                                                                 | ghostty-web                                                     |
| -------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `write` with completion callback                                                       | present, but parses synchronously; callback fires on next frame |
| `onData`, `paste`, `onResize`, `onKey`, `attachCustomKeyEventHandler`, selection calls | present                                                         |
| `scrollToBottom`, `loadAddon`, `FitAddon`, `element`                                   | present                                                         |
| `onBinary`                                                                             | missing                                                         |
| `parser.registerOscHandler`                                                            | missing                                                         |
| `buffer.active`                                                                        | missing                                                         |
| `refresh`, `options`                                                                   | missing                                                         |
| `@xterm/addon-clipboard` for OSC 52                                                    | not applicable; unknown whether OSC 52 is handled internally    |

Consequences for Farhelm's specific design, in rough order of how much work each implies:

The backpressure model changes. The watermark scheme in SPEC_impl.md rests on xterm's asynchronous write queue and its
50 MB silent-discard cap, and the audited parse-rate numbers are xterm's. ghostty-web parses synchronously inside
`write`, so the WebSocket handler itself becomes the bound. The pause/resume protocol to the supervisor stays, but what
triggers it has to be re-derived, and that section of SPEC_impl.md rewritten. The "no Farhelm-owned code may ever drop
a terminal byte" invariant still has to hold and would need re-auditing against a different buffering story.

`onBinary` and the OSC handler need replacements. Mouse reports would presumably arrive via `onData`. The
OSC-handler-returns-a-promise mechanism has no hook to attach to, and OSC 52 clipboard support would have to be checked
end to end.

The scroll-freeze workaround goes away, which is a deletion, not a port. But ghostty-web auto-scrolls to the bottom on
every write (a deliberate divergence from xterm, per a comment in its `writeInternal`), and the scroll tests in the
browser suite will notice.

Reconnect prefill and cursor resynthesis are plain `write` calls and should carry over.

## Effort estimate and recommendation

Medium to high. Calibration: this is a reading-the-code estimate, nothing was prototyped.

A spike of one or two days would mount ghostty-web in the island, feed it the WebSocket, and see what breaks, with WebKit
the first thing to check since that is where the desktop lives. A real migration is a few weeks: the four missing hooks,
re-deriving backpressure and rewriting its SPEC_impl.md section, a provenance answer for the wasm blob, and re-running
the browser suite in Chromium and WebKit.

The payoff is deleting the scroll-freeze workaround and getting correct grapheme and SGR handling (XTPUSHSGR/XTPOPSGR,
complex scripts) from the same emulator Ghostty itself runs. Whether that is worth it depends on how much the xterm
defects hurt in practice, which on this date was "an annoying but worked-around scroll bug". Parked as maybe-later.

## Sources consulted

- https://mitchellh.com/writing/libghostty-is-coming
- https://x.com/mitchellh/status/2088378990998524206 (wasm performance vs xterm.js)
- https://x.com/mitchellh/status/1981113067238048013 (libghostty-vt as a standalone wasm module)
- https://github.com/coder/ghostty-web and its `lib/terminal.ts`, `CHANGELOG.md`
- https://github.com/ghostty-org/ghostling
- https://github.com/ghostty-org/ghostty `include/ghostty.h`, `include/ghostty/vt/`, and the `tip` release assets
- https://news.ycombinator.com/item?id=46110842
- https://wterm.dev/ghostty (another libghostty-vt web wrapper, DOM renderer; not evaluated further)
- https://github.com/Uzaaft/awesome-libghostty
