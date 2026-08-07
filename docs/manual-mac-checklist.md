# Mac close-out checks

NOTE: These are manual checks, not CI gates. Playwright's WebKit build does not run inside WKWebView, and remote-paste
latency needs a real Mac-to-host link. Record the observed facts and timings here when the release candidate is run.

## Native-app release close-out

Run these seven steps against the same release candidate and record failures with the app build, Mac model, macOS
version, and remote Ubuntu version. A pass here is evidence about that exact candidate, not a substitute for the Linux
and browser CI gates.

1. Start the native app and provision a fresh Ubuntu host using only the account's existing passwordless SSH. Confirm
   that setup needs no root, the supervisor registers, and a session runs. Then use
   `Farhelm.app/Contents/MacOS/farhelm helm token show` and open the same embedded helm's authenticated web UI.
2. In an existing `jj` workspace where Git reports detached HEAD, create an official Claude Code session in one action.
3. Create a local Mac session the same way. Confirm the local and remote sessions appear together in one list.
4. Paste a Mac screenshot into the remote terminal. Confirm the path appears at the active cursor and Claude can read
   the file; also complete the clipboard-facts and latency records below.
5. Quit and relaunch the app. Confirm both sessions and their terminal state remain. Reboot the Mac and confirm the
   remote session is untouched while the local session is interrupted and offers conversation resume.
6. Attach to the remote session from the token-authenticated web UI and confirm the native app visibly detaches.
7. Ask real Claude to create a new `jj workspace` and invoke the injected spawn CLI. Confirm the child appears without
   refreshing either client.

Observed release/build: not run

Mac and remote environment: not recorded

Close-out result: not run

## Clipboard file names

- Create a test image with a deliberately non-sensitive name and timestamp, then start `Farhelm.app`, open a local or
  remote terminal, and copy that file in Finder. Do not use an ordinary work or personal file for this check.
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
