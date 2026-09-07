# Browser evidence contracts

These focused contracts validate the test timeline without starting Farhelm. Run them on a browser worker from `e2e/`,
using the run recorder as described in [the narrow-test recipe](../../.agents/narrow-tests.md):

```sh
python3 harness-tests/test-supervise-child.py -v
npx playwright test --config=harness-tests/playwright.config.ts --workers=1
```

The Playwright selection covers Chromium and WebKit, including eight intentional children whose failure, timeout,
unexpected pass or teardown failure must retain parseable timelines and trace observations. A successful parent removes
its child's artifacts; failed verification keeps them beneath that parent's output directory. These contracts stay
outside ordinary `*.spec.ts` discovery and do not add hosted CI runs.

The child runner bounds each output stream and inspected trace data. Its Python supervisor retains the CLI's wait
identity through group cleanup. On Linux, subreaping also owns orphaned detached browser processes; other platforms
report that detached descendants are outside group cleanup. Killing the supervisor itself with SIGKILL cannot run its
cleanup. The tests' own finite fixture lifetimes provide a fallback when testing a broken supervisor.

The timeline records bounded metadata, ordering and explicit loss; it omits typed text, frame payloads and credentials.
Field inputs are indexed key/value pairs: the collector reads only its admitted prefix, so a wide caller collection
cannot trigger whole-object key enumeration before the field cap takes effect. Helper records retain their
observer-owned page identity. Each document carries a random token through fragment and history changes, independent of
fake clocks; missing document-start evidence remains explicit loss. Adopting an already open context observes its pages
immediately, but cannot recover events from documents that predate script installation. An exhausted timeline cannot
prove event absence. A killed Playwright worker may bypass fixture finalization, leaving the enclosing run recorder as
the available evidence. Trace inspection caps selected event streams, not the entire Playwright trace archive.

For helper changes that need the real product stack, these existing selections exercise focus refusal, held replay and
manual-context takeover. Each names both engines without widening to the full product battery:

```sh
npx playwright test --project=chromium-profiles --project=webkit-profiles -g 'a popup-vetoed terminal reveal requires a later click$'
npx playwright test --project=chromium-terminal-replay-rename --project=webkit-terminal-replay-rename -g 'replay-degrades-on-detach: a takeover mid-catch-up shows what arrived, under the banner$'
npx playwright test --project=chromium-terminal-reconnect --project=webkit-terminal-reconnect -g 'manual-reconnect-takes-the-session-back$'
```

Inspect retained timelines and traces when these fail. Passing counts alone do not establish that observation remained
passive or was attributed to the intended page; the standalone contracts and source review cover those claims.
