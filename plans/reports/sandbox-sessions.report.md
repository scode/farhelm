# Sandbox sessions

## Since the last review

The same three draft PRs, #3263–#3265, are retained. They were carefully rebased onto current main without changing
their own diffs. The final PR now replaces the completed Phase 2 TODO entry with the remaining follow-ups. Usable
private API access allowed the previously blocked real-sandbox verification to finish. The simplified ownership model
recorded in the latest Decisions remains the implementation.

## What this was about

A session can now launch into its own disposable Tensorlake sandbox from a recipe, directly or through a template. Clone
creates another sandbox from that recipe. Individual Suspend, Resume, Stop and suspend, and deliberate Delete control
its lifecycle. Ordinary Tensorlake sandboxes added as hosts remain supported.

The original plan grew mechanisms for proving which sessions remained after a failed Replace. The maintainer replaced
that design with whole-sandbox Delete, preservation of existing work when Replace fails, and agent refusal in the helm.
The drafts implement that smaller contract; the obsolete membership, handshake, supervisor-policy and notification
machinery is removed.

## Things you should know

- **Delete destroys the entire sandbox:** every session and process, files, checkout, unpushed work and snapshots,
  including an unlisted session left by a failed Replace. Its confirmation is always shown. It works while suspended or
  disconnected and does not Resume to count sessions. Failed cleanup stays visible for an explicit retry.
- Replace stays in the existing sandbox and removes its source session only. A failed or ambiguous Replace preserves the
  sandbox and files, identifies it, and reports what is known. Rare ambiguous cases may require manual recovery. A
  failed ordinary first launch may discard only the new sandbox created for that launch, even after a lost reply.
- Plain Suspend preserves processes. The fixed 30-minute idle timeout starts after the connection from Farhelm ends; an
  open helm connection counts as activity, so a forgotten connected session can keep billing. Snapshot storage also
  bills while suspended. Automatic resume is disabled.
- Session-owned sandboxes appear in Hosts and the existing update, problem and progress area, with ordinary update
  eligibility. They have no permanent navigation-rail icon and are not ordinary destinations for unrelated launches.
- Agents in sandbox sessions cannot ask Farhelm to act. This is enforced in the helm, with the existing refusal when no
  owning helm is attached; it adds no bell notification or persistent supervisor policy. Other agents cannot create or
  destroy billed sandboxes through these sessions or recipe templates.
- This is experimental. Claude subscription logins may encounter first-run setup in each new sandbox. The updated
  Tensorlake sandboxes documentation explains launch, billing and deletion. No unattended preview server was started.

Real verification used **four sandboxes out of the approved whole-plan limit of 100**: one builder, one preparer, the
original session and its Clone. All four are terminated, no attributable snapshots remain, and the test SSH registration
was removed. The isolated test helm was stopped and its credential-bearing state removed. The live installation was not
used.

## Open questions and possible follow-ups

There is no blocking question. The maintainer-approved deferrals are recipe recent setups, Save as template from a
sandbox launch, and bulk Suspend, Resume and Stop and suspend. Recipe templates and individual controls work now. These
conveniences are tracked in TODO.md, alongside per-repository warm snapshots with incremental fetch, session resize
using the future shared host support, and the possible shared scratch volumes and warm forks. The existing agent-trust
and automatic-suspend follow-ups remain. Neither a per-launch size override nor a new recovery framework was introduced.

## The PRs

- [#3263](https://github.com/scode/farhelm/pull/3263/changes): the amended product and implementation contracts.
- [#3264](https://github.com/scode/farhelm/pull/3264/changes): ownership, recipe launch, cleanup and helm-side agent
  policy.
- [#3265](https://github.com/scode/farhelm/pull/3265/changes): launcher and lifecycle UI, templates, browser coverage,
  documentation, the experimental changelog entry and final TODO follow-ups.

## Checks run, reused and skipped

Current runtime checks used source `05488a7fcbea087b61cc8893cc390c42d6ded44a`, fingerprint prefix `dfe041b1b8d1…`. Only
TODO.md prose changed afterward. All 18 current recorder records have complete source capture, output EOF, no omitted or
truncated bytes, and no forced cleanup: **11 commands passed and seven private verification attempts failed**. These
command counts are separate from reported test-case counts.

**Run now:**

- `cargo build -p farhelm --bin farhelm` and pinned Dioxus 0.7.10 web build passed. The isolated cloud checks used that
  developer build on compatible Ubuntu through the supported explicit payload source; this was not release-artifact
  validation.
- `record-test-run.py --runner playwright … -- npx playwright test 'tensorlake-sandbox-sessions\.spec\.ts'`:
  `228ebe13-6b70-41ed-9313-2b656c85ce10`, **10/10 passed**, five cases on each of Chromium and WebKit, no retries or
  skips. This covers recipe consent and refusal, template placement, Resume and Stop-and-suspend questions, suspended
  controls, mandatory whole-sandbox Delete, Hosts visibility and permanent-rail absence on the merged UI.
- `record-test-run.py … -- deno run -A <private verification helper> <phase> <isolated helm>` retained these successful
  real phases:

  | Phase                                                                                                          | Recorder run                                                                   |
  | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
  | Account Test; SSH registration and recipe setup                                                                | `d76e2ea2-e752-466a-a49c-e87f3646fe29`, `446b4266-8c95-45e9-b15c-2afdd09bd1ed` |
  | Recipe build; managed public checkout and command launch                                                       | `5b1d5b40-0a3b-4ff5-b289-9d729db9c07b`, `fd56a3e8-6f9b-4463-a57d-e85da83f981c` |
  | Same process identity and PID, with fresh grid input/output after Suspend/Resume                               | `a0a028a8-4d24-4484-a86f-d4ab9085f366`                                         |
  | Provider confirms timeout 1800 seconds and automatic resume off                                                | `b1fb86c2-d02a-47f5-be01-1c0ea17b5077`                                         |
  | Existing GUI-created Clone: separate sandbox, same launch/repository, original-only file absent, fresh process | `e25de53d-9c37-4630-8d1b-f1b04a833008`                                         |
  | Owned resource/snapshot inventory                                                                              | `5752a3fc-5add-4731-b1cd-4c41caa2faba`                                         |
  | Confirmed GUI Delete of both original and Clone while suspended                                                | `196008a5-42aa-4c08-bcbc-bf93acaadeb9`                                         |
  | Recipe/snapshot cleanup, four terminated resources, SSH-key removal and test credential clearing               | `dc2875dd-b1ac-4021-ba75-e7a3fdef3c00`                                         |

- Seven failed private attempts are retained, not counted as passing commands. `176a37a0-5f56-4fee-b5fe-441475d6ec47`
  observed progress after setup had ended. `19dc9a16-761e-4e89-b273-a8e53d2d569d` wrongly expected literal newlines in
  an intentionally escaped inherited-command field, before creation. `2c4898bb-8e92-4ce9-acd7-e2daad1a03eb` successfully
  submitted GUI Clone but tried hovering behind its composer backdrop; the existing Clone was then verified without
  creating another. `f23f214c-dc02-433f-a09a-9ccc97f60fc9` observed the owned Update attention row and full hover card
  with 11 pending steps but asserted a running step too early; `ffdf11a0-cbb3-4ad1-85f1-bc68437080df` arrived after that
  Update ended. No running-step observation is claimed from those attempts. `6b79a818-7dde-4dce-9d93-5973b3954b77`
  successfully requested Stop and suspend but expected a fresh exited status from the frozen cache. After an explicit
  Resume, `c0e7fbeb-8908-4429-9e89-cd6b91dd8b52` proved the fresh exited status and the actual process's absence,
  suspended it again, then failed on an unrelated blank-page navigation mistake. The separate successful Delete and
  provider cleanup checks followed. No unresolved product failure was identified from these fixture mistakes.
- Changed Markdown formatting, changelog format and agent-instruction checks passed. The latter retains one existing
  warning about a stale heading citation in TRIAGE_OUTCOMES.md; it reports zero errors.

**Reused:** four prior recorded selections, **131 native cases passed**, on the reduced stack before this
byte-preserving rebase. `e3ae6939-0944-485e-bef7-b7b226ecc32c` covered 11 backend ownership, cleanup, agent-policy and
migration cases (source `197aea68…`); `d545f934-34fd-4c6c-8dd3-2eb84c05cdba` covered three launch-preview/preflight
cases (`19ce927b…`); `82f5ae37-6b9e-4a3c-90a6-5354384c3501` covered 114 selected UI logic cases (`1fd48f0b…`);
`2383e15c-5875-4517-b96c-f3fc7a3172d0` covered three API/operation cases (`3ddbdf26…`). Their complete metadata and
outputs were checked, with no runtime skip markers. Backend inputs are unchanged. Incoming main changed UI progress and
approval presentation; its integration was inspected independently, and the current two-engine sandbox run covers the
combined UI.

**Skipped:** no full Rust, browser or desktop battery, CI dispatch, installer/release validation, website rebuild or
live-install test. Clippy for the touched crates and shipping CLI, desktop compilation, Rust formatting and the
test-delay checker were not rerun: the earlier executor recorded passes, but their exact tested trees were not recovered
in this round, so they are not counted as current validation. The remaining change is TODO prose, the implementation's
own diff survived the rebase unchanged, and the targeted browser and real lifecycle checks address the remaining risks.
The documentation adds ordinary prose and links on an existing page; its new internal link targets and section anchor
were checked directly. Failed browser commands from earlier rounds are not reused as passes.

## Review gate outcome

The required single-reviewer gates used fresh-context Opus 5.5 at high effort. The reduced specifications passed, and
the backend follow-up passed after its findings were fixed. The UI follow-up cleared every material product finding and
left one test-fixture item: marking cached sessions stale while presenting their host as suspended. That fixture was
corrected, the original badge assertion retained, and a current Opus 5.5 high-effort read-only closure returned PASS
after reading the complete spec and checking the current browser evidence. This closure was limited to that item, not a
second review of all product code. The final TODO amendment also passed Opus 5.5 at high effort after its findings were
addressed. No obsolete membership or supervisor-policy guarantee was restored.
