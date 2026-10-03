### What this was about

Setting up or updating a host from the hosts panel ends with a step that waits up to 30 seconds for the helm to connect
to the supervisor the run just installed. That step only recognized success. When the supervisor answered but the helm
refused it (it speaks another protocol version, reports an identity other than the one on record, reports no identity,
or reports an identity another host entry holds), the host stayed busy for the full 30 seconds and the run then failed
as "timed out", while the hosts panel already showed the real reason. A machine that was reinstalled is the likely
trigger.

In triage you asked for the step to stop early and name the refusal. That was deferred because the step must not stop on
a refusal the host held before the step started, most often the protocol mismatch an update exists to fix; otherwise
every update of an outdated host would fail at once. You then chose to number the helm's connection attempts, with one
counter shared by every host, so the step can tell a refusal that answers its own reconnect from an older one. You also
agreed that the message reuses the wording the hosts panel and the update's own safety check already use.

The PR does that. Every connection attempt the helm makes now takes the next number from one shared counter just before
it starts, and a refusal is recorded with the number of the attempt it answers. When the attach step asks the helm to
reconnect, it gets back the highest number handed out so far, and only a refusal numbered above that stops the wait. The
run then fails at once with a message like "step 9 (attach-supervisor) failed: the helm refused the supervisor this run
installed: protocol version mismatch: peer speaks v34 (build "…"), this side speaks v35 (build …); update the farhelm
binary on outdated.example (or this helm) so the two speak the same protocol version; rerun provisioning to continue".
The other three refusals name the identities involved and the same remedies the hosts panel gives. Success, the
30-second budget for everything else, and the "host disappeared" failure are unchanged. The numbers are internal:
nothing the hosts panel or the API shows changed.

### Things you should know

- **Two gaps in the tests, by necessity.** The order that makes this safe has three parts: the helm reads the highest
  number before it sends the reconnect request, each attempt takes its number before it starts, and only a higher number
  counts. An attempt goes through three moments: it starts, its answer comes back, and later the helm records that
  answer as the host's state. The tests prove that a refusal whose answer came back before the reconnect never counts,
  even when it is recorded after the reconnect (the tests fail if the number is taken at that recording moment), that
  the reconnect's own attempt does count (including when the host's connection worker had died and had to be restarted),
  and that an outdated host's update still completes. Two wrong orderings cannot be shown failing on the single-threaded
  test setup: reading the highest number after sending the reconnect request instead of before, and taking an attempt's
  number when its answer comes back instead of when it starts. Both are documented at the exact lines, which is the
  protection they have.
- **No early stop when the host's connection is retired** (it disappeared from the registry or its worker crashed); that
  case keeps today's timeout. The plan made it optional and your decisions said it gets no new handling.
- **One rare case falls back to the timeout:** if the host's connection worker had died and another request (a Retry
  from the hosts panel, say) restarted it at the same moment as the attach step, the step does not stop early on a
  refusal; the user sees today's "timed out" after 30 seconds. It is never a wrong early stop.
- **A first version of one test proved nothing.** The test that an outdated host's update still completes passed even
  with the fix deliberately broken, because the host reconnected before the attach step started. It now switches the
  test host to the new protocol at the run's own "restart the supervisor" step, the way a real update does, and it fails
  when the fix is broken.

### Open questions and possible follow-ups

None that need a decision.

### The PRs

- #1498 `fix(helm): fail a host's attach step at once when the helm refuses it`:
  https://github.com/scode/farhelm/pull/1498/changes

### Checks run, reused and skipped

- Run on the final tree: `cargo fmt --all -- --check`; `cargo clippy -p farhelm-helm --all-targets -- -D warnings`
  (`farhelm-helm` is the only crate whose code changed); `cargo clippy -p farhelm --bins -- -D warnings`; the helm's
  connection-manager, provisioning and hosts tests through the recorder (`cargo nextest run -p farhelm-helm`, selection
  'helm manager, provisioning, hosts modules', run e927b612, 288 passed); `scripts/check-test-sleeps.py` (no unannotated
  delays); `dprint check` on the changed Markdown; `python3 releasing/check-changelog.py format`.
- Also done: deliberately broken versions of the fix, to confirm the new tests fail against them (taking a refusal's
  number when the helm records it instead of when the attempt starts, and dropping the number comparison). The tests
  failed against both, as they should.
- Reused: none.
- Skipped: the full workspace test suite and the end-to-end and browser tests. The change is confined to the helm's
  connection manager and the provisioning attach step, both covered by the module tests above, and nothing the UI or the
  wire protocol sees changed.

### The review gate's outcome

A fresh-context Opus 5.5 adversarial review (the plan asked for high effort; the sub-agent mechanism could not set
effort, so it ran at the executing session's own) found no correctness defect after tracing every point in a
connection's life at which the reconnect can land: in the middle of an attempt, waiting between attempts, idle, stopped
while it waits for you to answer an identity question, and with a dead connection worker. Its smaller findings (a
documentation inaccuracy, a test that could pass without testing anything, the wording of the protocol-mismatch and
identity messages, and a wrong code comment) were all fixed; the two test gaps described above are the part that could
not be closed.
