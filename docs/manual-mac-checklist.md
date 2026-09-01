# Mac close-out checks

NOTE: These are manual checks, not CI gates. Playwright's WebKit build does not run inside WKWebView, and remote-paste
latency needs a real Mac-to-host link. Record the observed facts and timings here when the release candidate is run.

## Native-app release close-out

Run these eight steps against the same release candidate and record failures with the app build, Mac model, macOS
version, and remote Ubuntu version. A pass here is evidence about that exact candidate, not a substitute for the Linux
and browser CI gates.

The release is two bare binaries in `~/.local/bin` (D6), and every step below starts `farhelm-desktop` from a terminal —
stderr assertions need a terminal, and the terminal path must keep working regardless of the bundle. But the installer
now also assembles `~/Applications/Farhelm.app`, so the launcher path needs its own close-out:

- After `install.sh`, `Farhelm.app` exists, Spotlight and (if installed) Alfred find "Farhelm" by name, and launching
  from there opens the window with the Farhelm icon in the Dock and "Farhelm" in Cmd-Tab.
- With the app already running, launching it again from Spotlight ACTIVATES the running window; it does not start a
  second instance (check with `pgrep -fl farhelm-desktop`).
- Quit, re-run `install.sh`, and confirm the bundle's `Contents/Info.plist` version tracked the installed version.

Know the failure shape a launcher launch cannot show: a preflight refusal (missing/old tmux, unusable state dir) prints
one line to stderr and exits, which a Finder/Spotlight launch swallows entirely — "nothing happens" from the launcher
means "run `~/Applications/Farhelm.app/Contents/MacOS/farhelm-desktop` in a terminal and read the line".

1. Confirm the tmux preflight and floor (SPEC_impl.md's "Terminal substrate: private tmux server") in three controlled
   launches.

   `farhelm-desktop` now owns this check itself: right before it would spawn its own managed supervisor, it probes the
   tmux it is about to hand that supervisor and applies the version floor. A missing or below-floor tmux refuses with
   ONE plain line on the app's own stderr and exit status 1 — no window ever opens, and nothing reaches a supervisor
   log, because the supervisor is never started. These cases only mean anything if the app is about to START a
   supervisor rather than reuse one: bootstrap discovers first, and an answering supervisor is reused exactly as it
   stands — tmux included — without this preflight running at all (see the separate check below). A stray supervisor
   left over from an earlier step would therefore make all three cases below pass without exercising anything. Before
   each launch, quit the app fully and confirm no supervisor is left:

   ```
   pgrep -fl 'farhelm supervisor run'   # must print nothing
   ```

   If it prints something, that is either a leftover child (wait for it, or kill it) or a supervisor you run yourself —
   in which case stop it for the duration of this step.

   (a) With Homebrew tmux installed and no override, the app starts and a local session runs:

   ```
   unset FARHELM_TMUX; ~/.local/bin/farhelm-desktop
   ```

   (b) Below the floor. Point the override at a distro or older Homebrew build and confirm `farhelm-desktop` itself
   exits 1 with exactly this on stderr, no window, and nothing on the supervisor's log (there is no supervisor log to
   check — it was never started):

   ```
   found tmux <version> at <path>, which is below the <floor> farhelm needs.
   On macOS, tmux has to be installed by hand; Homebrew is the recommended way:

       brew install tmux

   FARHELM_TMUX is set and overrides that search, so update it to point at the new install (or unset it) before
   starting farhelm-desktop again.
   ```

   ```
   FARHELM_TMUX=/path/to/old/tmux ~/.local/bin/farhelm-desktop
   ```

   (c) The configured tmux is missing. Name a path that does not exist:

   ```
   FARHELM_TMUX=/nonexistent/tmux ~/.local/bin/farhelm-desktop
   ```

   A nonempty override is what makes this case reachable at all. Shortening `PATH` does not: with no override the app
   probes `/opt/homebrew/bin`, `/usr/local/bin` and `/opt/local/bin` by absolute path, finds the Homebrew tmux case (a)
   just established, and starts normally. So this case tests a configured-but-absent binary, which is the realistic
   version of "no tmux" for a Mac that has Homebrew — not an empty machine. Confirm the same shape of refusal as (b),
   with the `NotFound` subject instead ("...and none could be run (looked at: /path/to/old/tmux). Each one was either
   not found, or is missing its interpreter or loader.").

   Record whether the ONE stderr line and exit status actually matched, and whether Finder — as opposed to a terminal
   launch — showed anything at all; a Finder launch has no terminal for that stderr to reach, which is the remaining gap
   TODO.md tracks. Drop the overrides before the remaining steps.

   Separately, confirm that an ANSWERING manual supervisor is reused untouched regardless of tmux: start one yourself
   (`farhelm supervisor run --state-dir ~/.local/state/farhelm &`) against a tmux of your choosing, then launch
   `farhelm-desktop` with `FARHELM_TMUX` pointed at something missing or below the floor. The app must start normally
   against your supervisor — the preflight must not run at all, because this process is not about to spawn or configure
   a tmux of its own.
2. Start the native app and provision a fresh Ubuntu host using only the account's existing passwordless SSH. Confirm
   that setup needs no root, the supervisor registers, and a session runs. Then use
   `~/.local/bin/farhelm helm token show` and open the same embedded helm's authenticated web UI.
3. In an existing `jj` workspace where Git reports detached HEAD, create an official Claude Code session in one action.
4. Create a local Mac session the same way. Confirm the local and remote sessions appear together in one list.
5. Paste a Mac screenshot into the remote terminal. Confirm the path appears at the active cursor and Claude can read
   the file; also complete the clipboard-facts and latency records below. Then check the COPY direction, which rides the
   native clipboard route (`POST /api/clipboard` → arboard; the webview itself has no clipboard API): mouse-select text
   in a terminal pane and paste it into TextEdit, and have the agent in the pane do an OSC 52 copy (Claude's own copy
   action, which prints "copied N chars") and paste that too. Both must yield the copied text, not the clipboard's prior
   contents — the exact failure shipped until 2026-09.
6. Choose a non-newest session and a non-default list order, then quit and relaunch the app. Before clicking anything,
   confirm that same session is selected and attached and that the chosen order is still applied. Also confirm both
   sessions and their terminal state remain. Reboot the Mac and confirm the remote session is untouched while the local
   session is interrupted and offers conversation resume.
7. Attach to the remote session from the token-authenticated web UI and confirm the native app visibly detaches.
8. Ask real Claude to create a new `jj workspace` and invoke the injected spawn CLI. Confirm the child appears without
   refreshing either client.

Observed release/build: not run

Mac and remote environment: not recorded

Tmux floor refusal message: not recorded

Close-out result: not run

## Clipboard file names

- Create a test image with a deliberately non-sensitive name and timestamp, then start `~/.local/bin/farhelm-desktop`,
  open a local or remote terminal, and copy that file in Finder. Do not use an ordinary work or personal file for this
  check.
- Paste it into the terminal. Expand `clipboard facts` below the terminal and copy the JSON dump here before navigating
  away. It records item order, kinds, MIME types, `File.name`, and `lastModified`. Before putting the dump in this
  tracked document, replace the test filename with `<test-file>` and every timestamp with `<timestamp>`; remove any
  other workstation-specific value rather than committing it.
- Confirm the uploaded path keeps Finder's filename. A genuinely synthetic screenshot may still use `pasted-N.ext`.
- A real image named `image.<ext>` and modified immediately before the paste is indistinguishable from WKWebView's
  synthetic placeholder under the current heuristic; record that false positive if it occurs.
- Repeat with a screenshot copied directly to the clipboard. Record whether WKWebView supplies a name and whether that
  name matches the engine-placeholder rule.

Observed release/build: not measured

Clipboard facts: not captured

## Remote-paste latency

- Open a session on a real remote host through the native Mac app.
- Paste a screenshot and measure from the paste gesture until the uploaded path appears at the terminal cursor.
- Record the link, approximate payload size, elapsed time, and whether the path appeared at the cursor that was current
  when the upload completed.

Observed release/build: not measured

Link and payload: not measured

Paste-to-path latency: not measured

## Terminal selection dismissal (carried from M5)

- PLAN.md's M5 entry records this as unconfirmed on real WKWebView: select-and-copy in a terminal creates BOTH an xterm
  selection and a native document selection over the DOM rows, and the fix that clears both was verified in Chromium and
  Playwright's WebKit but still painted on the macOS desktop app when the M5 manual round ran out of time.
- In the native app, select terminal text, copy it, then type and paste. Confirm no selection highlight survives any of
  those actions. If one does, the remaining suspect is WKWebView holding its selection layer after the ranges are gone —
  record exactly which action leaves it painted.

Observed release/build: not run

Selection result: not recorded
