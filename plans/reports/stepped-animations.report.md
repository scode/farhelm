### What this was about

On a Mac with a 120 Hz display, the desktop app kept the macOS window server at roughly 40-50% CPU and drained the
battery whenever sessions were running, even with the terminal idle and the Farhelm window hidden. You confirmed the
cause: turning on Reduce Motion made the load go away. The culprit was the green dot that pulses beside a running
session. It faded smoothly, and a smooth looping animation makes macOS redraw the whole window on every display refresh
for as long as it runs.

You decided: the specs forbid that animation style in words an agent can apply without interpreting "smooth"; every
looping animation (not only the pulse) changes in steps, at most 10 times per second; all animations stop while the
window is not the one in use (no keyboard focus, or hidden or minimized, so a Farhelm window visible beside another
focused app counts as inactive); Reduce Motion keeps everything static; and the proof is tests, with the real
window-server effect checked by you on macOS after merge.

The PR does all of that. The running pulse now walks through eight steps per 2-second cycle (four changes per second,
same 1 to 0.45 opacity range), and the host update dot and the delete-in-progress dot reuse it. The delete spinner turns
in eight 45-degree jumps per 0.8 seconds, ten changes per second, a classic stepped spinner. One CSS rule pauses every
animation on the page while the window is inactive, driven by a few lines of page script that track the window's focus
and visibility. SPEC_impl.md ("GUI: Dioxus") gains the rule stated by its effect (nothing may redraw on its own more
than 10 times a second; the forbidden techniques are listed as examples, not a whitelist) with the reasoning and your
observation; SPEC.md's status section gains one sentence. A new JS unit test file (15 cases) fails if the app's
stylesheet gains a looping animation that interpolates or changes more than 10 times a second. The TODO entry "Running
sessions keep WindowServer busy and drain the battery" is removed, and a changelog fragment describes the fix for Mac
users.

### Things you should know

- **The pause freezes one-shot animations too.** The review pointed out that the one rule which pauses everything also
  freezes a short one-off animation that happens to start while the window is inactive; an entrance that starts
  invisible would stay invisible until you come back. None exist today. The spec and the stylesheet comment now say so,
  and tell the next author to use a CSS transition (which the pause does not touch) for such effects.
- **The pause also stops the terminal's blinking cursor** while the window is inactive. The cursor only blinks while the
  terminal has focus anyway, so this should not be visible.
- **The pause has no automated test.** Browser test runners report the page as focused no matter what, so a test would
  either fail against correct code or push the code into trusting events over the browser's own focus state. This was
  decided in the plan; your macOS check is the real test.
- **A small placement change from the plan.** The plan suggested setting up the focus tracking at the very top of the
  UI. It is set up one level down instead, in the part of the UI that both the web UI and the desktop app show after
  sign-in, because the desktop app's top level has a startup step that must stay first. The behavior is the same.

### Open questions and possible follow-ups

- **Worth checking in your macOS session: the dots right after launch.** The pause follows the page's own focus state,
  which on macOS tracks whether the embedded web view has keyboard focus inside the window, not just whether the window
  is in front. If the app ever comes up without the web view focused, the dots would stand still until your first click.
  I expect the window launches focused (the app already brings itself to the front on launch), but this could not be
  verified without a Mac. Also worth a look: switching away with Cmd-Tab and back. If the dots stay frozen in either
  case, the fix is to also update on the first key press or click.
- **Whether stepped animation actually lets the window server idle** is your post-merge check, as agreed. If it does
  not, that is a follow-up, not part of this plan.

### The PRs

- #1481 `fix(ui): step looping animations and pause them while inactive` (draft):
  https://github.com/scode/farhelm/pull/1481/changes

### Checks run, reused and skipped

- Run: `cd crates/farhelm-ui/js-tests && node --test` (173 pass, including the 15 new ones); a deliberate revert of the
  pulse to its old timing makes the new test fail.
- Run: `cargo fmt --all -- --check`; `cargo clippy -p farhelm-ui --all-targets -- -D warnings` (only that crate
  changed); `cargo check -p farhelm-ui --features desktop` (the listener install compiles in both builds). The only Rust
  change after these ran was a comment.
- Run: `dprint check` on every changed Markdown and CSS file; `python3 releasing/check-changelog.py format`.
- Reused: none.
- Skipped: browser end-to-end tests (agreed in the plan: they cannot observe focus loss, and no existing browser test
  depends on animation timing; the reviewer checked that the two browser tests reading animation names are unaffected by
  pausing); the Rust test battery (the only Rust change is one page-script install with no Rust-side behavior).
- Not run: any measurement of the window server; that is your check.

### The review gate's outcome

A fresh-context Opus 5.5 review (the plan asked for a high-effort review; the sub-agent mechanism used could not set
effort, so it ran at the executing session's own) found no blocking defects and confirmed every acceptance criterion,
including the timing math, the cascade against the Reduce Motion overrides, and that the pause wins over every
animation. Of its seven low-severity findings, six were fixed: the one-shot-animation note above; the new test silently
passing a vendor-prefixed animation; the new test undercounting two rare animation shapes (now refused or counted
conservatively); the test's handling of comments and escaped quotes; and two comment wordings, counted as two findings.
The seventh, the launch-focus question, needs a Mac and is listed above.
