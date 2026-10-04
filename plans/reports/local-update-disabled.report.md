## What this was about

In the hosts list, every host row's `⋯` menu has an **update** item that makes the helm install the newer Farhelm on
that host over ssh. On the row for the helm's own machine (**local (this machine)**), choosing it got a refusal ("this
is the helm's own machine; run farhelm helm setup here instead of provisioning from the panel"), and that error then
stayed under the row with no way to dismiss it. You asked for the item to be greyed out on that row instead of hidden,
with its description pointing at the installer, so the refusal is never reached.

When the run first started, I stopped to ask about Linux. SPEC.md still supports a Linux machine running a helm, and the
installer refuses Linux, so pointing at the installer would be wrong there. You answered "We are only targeting Mac OS
right now. It's fine." I took that as: name only the installer, with no per-platform text.

## Things you should know

- On the local row, **update** now shows greyed out with the line "run the installer again to update this machine".
  Clicking it or pressing Enter on it sends nothing. It shows wherever the menu would otherwise have offered update,
  which includes a local row that is already up to date. That is why the line says how this machine is updated rather
  than suggesting an update is waiting.
- Remote rows are unchanged, including the inline **↑ update** button and **update all**. The local row was already kept
  out of both. That exclusion now rests on a new, explicit "can the panel update this host" check rather than only on
  the check for whether an update may start without a confirmation step, which used to exclude it by coincidence.
- If a failed update of the local row were ever left over, its "try again" item is now greyed out too, since the helm
  would refuse it the same way. I believe no current helm can produce such a run.
- The helm side is untouched: it still refuses a local update, with the same wording, for any other caller.
- Your literal request also said "in the inline update button". The local row has never shown that button, and SPEC.md's
  host-list paragraph says local hosts keep the plain words. So I did not add a greyed-out inline button, which would be
  new UI that contradicts that paragraph.
- The new SPEC.md sentence, in the host-list paragraph: "The helm's own machine is not updated from the panel: its row
  menu shows Update greyed out wherever it would otherwise be offered, saying to run the installer again, and choosing
  it sends nothing." The same document still says a Linux helm is supported; per your answer, that mismatch is accepted.
- The website's Manage hosts page gains a short paragraph saying the Mac running Farhelm is updated by running the
  installer again, with a link to the Install page. There is a changelog fragment, and the TODO entry "No update for
  this machine" is removed.

## Open questions and possible follow-ups

- On a Linux machine running a helm, the greyed-out item tells you to run the installer, which refuses Linux. Nothing
  else changes there. Once the installer supports Linux again the text becomes right. If Linux helms come back into
  scope first, the text would need a Linux variant.

## The PRs

1. https://github.com/scode/farhelm/pull/1558/changes `fix:` grey out update for the helm's own machine in the hosts
   list. One PR, as planned; the description is empty because the title and diff carry it.

## Checks

Run on the final commit. Brackets hold the start of each run's id in the test-run recorder's retained records.

- Rust: the hosts-list and provisioning tests in the UI crate, 49/49 [4c9e82cd]. These include a new unit test that pins
  the item's disabled state and text for the local and remote rows. Clippy and `cargo fmt --check` clean.
- Browser, Chromium and WebKit: a new test, plus the two neighbouring local-row menu tests, 6/6 [9850b940]. The new test
  checks the item is visible, marked disabled, and carries the installer line, and that a real click and Enter on it
  send no update request.
- I proved the new test can fail. With the item left enabled, it fails on the disabled check [2dd6f48f]. With only the
  click guard removed, it fails on the request count, on both engines [4ddbc392].
- The test-sleep checker (both new waits are annotated observation windows), dprint on the changed Markdown, the
  changelog format lint, and the website build (all internal links valid): clean.

Skipped: the rest of the browser suite and the workspace Rust battery. The change is one menu item's rendering and its
guard in the hosts list. The checks above cover it directly, and nothing touches the helm, sessions or other panels.

## Review gate

One review round by a fresh-context Claude Opus at high effort, as the plan required. It found no correctness bugs. It
made six smaller points, all applied:

- assert focus before pressing Enter in the test;
- correct the test comment on how a request would be sent, and widen its wait to one second;
- document the greyed-out "try again" item;
- base the inline button and **update all** exclusion on the explicit "can the panel update this host" check;
- scope the SPEC.md sentence to where update would otherwise be offered;
- use a real click instead of a synthetic one.

I did not run a second round. None of the six changed behavior beyond the test itself, and I re-ran the evidence on the
result.
