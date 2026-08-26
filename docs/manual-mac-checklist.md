# Mac close-out checks

NOTE: These are manual checks, not CI gates. Playwright's WebKit build does not run inside WKWebView, and remote-paste
latency needs a real Mac-to-host link. Record the observed facts and timings here when the release candidate is run.

## Native-app release close-out

Run these eight steps against the same release candidate and record failures with the app build, Mac model, macOS
version, and remote Ubuntu version. A pass here is evidence about that exact candidate, not a substitute for the Linux
and browser CI gates.

The release is two bare binaries in `~/.local/bin` (D6), so every step below starts `farhelm-desktop` from a terminal.
That is not a compromise for the sake of the checklist — it is how the binaries are installed and how the maintainer
runs them — but it does mean this pass says nothing about a Finder double-click, which was never validated either.

1. Confirm the tmux discovery and floor (SPEC_impl.md's "Terminal substrate: private tmux server") in three controlled
   launches.

   These cases only mean anything if the app STARTS the supervisor rather than finding one. Bootstrap probes first and
   reuses whatever already answers, and a reused supervisor inherited its tmux from whoever launched it — so a stray one
   makes all three cases pass without exercising anything. Before each launch, quit the app fully and confirm no
   supervisor is left:

   ```
   pgrep -fl 'farhelm supervisor run'   # must print nothing
   ```

   If it prints something, that is either a leftover child (wait for it, or kill it) or a supervisor you run yourself —
   in which case stop it for the duration of this step. Discovering and reusing an existing supervisor is a separate
   behaviour worth checking on its own; it is not a substitute for these three.

   (a) With Homebrew tmux installed and no override, the app starts and a local session runs:

   ```
   unset FARHELM_TMUX; ~/.local/bin/farhelm-desktop
   ```

   (b) Below the floor. Point the override at a distro or older Homebrew build and confirm the supervisor refuses with
   `tmux <version> at <path> is below Farhelm's floor <floor> (see README: tmux)`:

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
   version of "no tmux" for a Mac that has Homebrew — not an empty machine.

   For (b) and (c), record where the refusal actually surfaced. Expected today, per TODO.md's entry: it reaches the
   supervisor's log and nothing else, so the window opens with a local host missing and no stated reason. Write down
   whether anything in the window explained it — that gap is what the TODO tracks, and this is the observation that
   would close it. Drop the overrides before the remaining steps.
2. Start the native app and provision a fresh Ubuntu host using only the account's existing passwordless SSH. Confirm
   that setup needs no root, the supervisor registers, and a session runs. Then use
   `~/.local/bin/farhelm helm token show` and open the same embedded helm's authenticated web UI.
3. In an existing `jj` workspace where Git reports detached HEAD, create an official Claude Code session in one action.
4. Create a local Mac session the same way. Confirm the local and remote sessions appear together in one list.
5. Paste a Mac screenshot into the remote terminal. Confirm the path appears at the active cursor and Claude can read
   the file; also complete the clipboard-facts and latency records below.
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
