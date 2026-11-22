## What this was about

When two test requests targeted the same worker checkout, the second copied its tools and source before checking whether
the checkout was busy. It could overwrite what the first run was using and then copy the first run's logs as its own. A
standalone setup could also change shared prerequisites while a different checkout was running tests.

## Things you should know

Each request stages its own files. The worker installs them under the checkout lock, keeping file mtimes and the build
cache for incremental runs. The lock stays with the worker when SSH disconnects. Setup waits for other setup calls and
refuses while tests are running. The driver prints an invocation ID at startup. Use it with
`scripts/test-worker/test-worker.sh collect --out <directory> <invocation-id>` to retrieve a detached run's evidence
after it finishes. The worker retains 20 invocations per checkout. A local output directory carries one invocation stamp
and rejects concurrent reuse.

Stop keeps both locks through recorder shutdown. It also waits when the foreground command exits before another process
in its group. If the group still runs after the 35-second grace period, the worker escalates and retains whatever
evidence was written.

## Open questions and possible follow-ups

The local stand-in covers the driver, file transfer, locking and process-group behavior. An SSH daemon and setup's
package installation are outside that fixture.

## The PRs

- [#4575](https://github.com/scode/farhelm/pull/4575/changes) protects staged input, active runs and collected evidence.
  Staging and detachment both needed evidence keyed by the same invocation ID, so they share one PR.

## Checks

- Ran `python3 -B scripts/test-worker/test-test-worker.py -v` on `39ac2a87543c`. All eight local scenarios passed,
  including a busy refusal with byte-identical source and tools, SSH-client interruption, bootstrap exclusion, output
  ownership and recovery, retention, and checkout-scoped Stop. Recorder `470e906b-3b55-4591-9e00-fae9e4f85f86`.
- Ran the command below on `e6ce5ed7758d`. All three cases passed with no skips. Recorder
  `e7817ac8-3dc0-40e5-ac15-775fb21edccd`. The earlier suite still covers the other paths. The later change recognizes
  live sibling threads when the main thread has exited. The rebases preserved the tested worker scripts through draft
  head `18180f0638c5`.

  ```sh
  python3 -B scripts/test-worker/test-test-worker.py -v \
    WorkloadStatTest.test_zombie_main_thread_does_not_hide_its_live_threads \
    WorkerTest.test_stop_keeps_both_leases_through_recorder_teardown \
    WorkerTest.test_forced_stop_leaves_a_collectable_invocation
  ```
- ShellCheck, Python byte compilation, scoped Markdown formatting, whitespace and the instruction checker passed. The
  instruction checker reported five pre-existing citation warnings.
- Skipped Rust, GUI and live worker runs. This change is to maintenance tooling and its committed fixture uses a local
  SSH stand-in.

## Review gate

Independent GPT-6.2 Sol high review and delta checks confirmed all findings were resolved. A separate scope review
confirmed the Stop implementation uses the recorder's group-ownership technique while allowing a normal run to finish
with a deliberately detached child.
