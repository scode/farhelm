# The uninstall notice copies a remote path without a hidden-characters warning

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

Additional reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

The kept-data notice displays a remote path with hidden characters escaped but copies its raw value without warning. An
ordinary successful uninstall rejects line breaks before reaching that notice. A different conditional route remains:
after uninstall fails, removal from another window can expose the owed notice before the first window receives failure.
A dangerous clipboard value and subsequent unsafe shell paste remain conditions, not observed exploitation.

## Details

Suggested bucket: **highest**, conditional remote-to-local security consequence. Confidence: **confirmed raw-copy
behavior and conditional concurrent-removal source route**; clipboard success, human reliance and downstream command
execution remain unverified. Hidden Unicode outside control characters is a separate remainder with no established
execution consequence. This record does not claim any arbitrary newline path survives successful uninstall.

`crates/farhelm-ui/src/hosts.rs:2283–2285` at the additional revision copies `notice.state_dir` raw without
hidden-character feedback. Escaped visible text does not change those bytes. The retained plan supplies the path. A
hostile host's inspection can supply the default path when no state folder is registered; the original review traced the
NUL-separated field through `bytes_path`. That input route alone does not prove notice reachability.

### Correction: successful uninstall rejects line feeds and Cc controls

The original finding incorrectly said provisioning had no later control-character guard. At the original reviewed
revision, `crates/farhelm-helm/src/provisioning/plan.rs:834–847,856` places `StopSupervisor` using the planned state
socket before `ForgetHost`, and returns that same kept-state path. `backend.rs:2634–2643` constructs the stop script
through `shell_path`/`path_text` (`2798–2799,2816–2817`), which delegates to `units.rs:471–482`: UTF-8 and
`char::is_control` validation. Failure stops execution (`service.rs:1996–2006,2226–2233`). At the additional revision
the equivalent paths are `plan.rs:842–855,864`; `backend.rs:2644–2653,2813–2815,2831–2833`; and
`service.rs:1995–2006,2225–2232`.

Confirmation compares the whole derived-equality plan (`service.rs:1847–1864`; `plan.rs:397–415`), preventing an
ordinary run from validating a clean replacement path while keeping a different original newline path in its notice.
Host/socket absence does not bypass local script construction. The notice normally appears on host absence
(`hosts.rs:1529–1548`); failure cleanup withdraws its owed entry (`provisioning.rs:3546–3570`). Independent review
agreed to refute only this ordinary completed-uninstall line-feed/control-character scenario. The earlier verification
sketch that stopped at plan construction was insufficient.

### Concurrent removal can still promote the owed notice

An accepted uninstall registers the owed notice, fails, and releases its server run lock. A second GUI can remove that
host before the confirming GUI receives the matching Failed progress. Host and progress reads are independent
(`list/view.rs:1278–1290,1732–1809,1843–1846,1902–1905`; `provisioning.rs:2738–2741,2810–2887`; `reader.rs:644–678`).
The HTTP funnel orders preceding configuration saves, not sibling GETs (`api.rs:1981–2051`). Accepted uninstall requests
progress without awaiting it (`provisioning.rs:1859–1868`).

The host read can therefore commit absence before any Failed snapshot. The notice effect promotes on absence alone
(`hosts.rs:1529–1548`). After removal, server progress needs the missing host row and returns an error
(`service.rs:2335–2358`; UI `api.rs:4494–4505`), which sets only `progress_error` (`provisioning.rs:2871–2876`). Even a
late matching failure cleanup removes only the owed entry, not a notice already promoted (`3550–3568`). No special
ordering between effects is needed when no Failed value was received at all. This establishes the conditional notice
route, not an observed paste exploit.

The guard also does not reject every presentation-unsafe character: `farhelm-proto/src/text.rs:18–36,57–79`
distinguishes characters such as U+202E, U+200B and U+2028 from Cc controls. Keep that raw-copy remainder without
assuming those characters have the line-feed command-injection effect.

Possible cover: OSC 52's accepted remote clipboard effect differs from an explicit GUI path-copy button.
`TRIAGE_OUTCOMES.md:2287–2309`, “session-header-raw-peer-text.md,” is precedent for retaining raw copy with a warning,
not acceptance of omitting it. The concurrent false-success decision, “uninstall-forgotten-success.md,” at pinned-tip
`12897–12924`, accepts misleading status, not remote-to-local command execution. No exact full exclusion was
established. Proposed drop: none for these remaining conditional concerns. A fix should follow the existing
hidden-character copy feedback contract; further validation must distinguish successful-run guards from concurrent
independent removal.

Original provenance: prioritized persistent review `161-ui-hosts-3of6/f1`, revision 2, with independent disagreement on
the OSC 52 exclusion. Corrections and extension: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`,
private run `4fae501c-3686-4822-aa6c-037266637632`, `g022/f5`, fresh `a013/f1–f3`. No runtime reproduction or clipboard
experiment.
