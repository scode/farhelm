# Typed SSH capacity reasons bypass final display escaping

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: a configured SSH alias with invisible/directional formatting can appear verbatim inside a
container host's recipe status and host-option diagnostic when a shared SSH channel is refused and the fallback login
fails. The UI does not visibly escape that part of the reason. Suggested bucket: other, misleading diagnostic spelling
and violation of the existing presentation contract. Confidence: confirmed conditional source/contract route, not
runtime-observed frequency. Could-be-highest:no established security, unauthorized execution, wrong target,
destructive-consent, data/process/work-loss or HTML-execution route. Actual routing retains the correct raw alias; only
its display is wrong. proposed_drop:false for this narrow current defect. Original proposed source-refutation of
arbitrary remote-error/security hypotheses remains recorded below and is not an accepted parent drop/lowering.
possible_cover:g059/l5 literal recipe-reference/account summaries and parent-supplied g086/l4 host-alias footer share
the policy but are distinct producers/sinks/fixes; neither is an alias for this record. No coverage-source search or
general escaping sweep performed. Contract: peer.rs:4-14 defines the boundary as AUTHORSHIP, not distance, and
explicitly includes transport errors, remediations and SSH destinations even when local. :49-55 applies final
presentation helpers to directly decoded proto and plain diagnostic snapshots. display_peer :159-180 makes unsafe
presentation characters visible. Thus the former statement that final escaping here was only future hardening is
superseded for this esta

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g077/f4`.

### g077/f4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 25 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. Final reopened synthesis under the original ID; original
consequences and proposed-drop bases preserved. TLDR: a configured SSH alias with invisible/directional formatting can
appear verbatim inside a container host's recipe status and host-option diagnostic when a shared SSH channel is refused
and the fallback login fails. The UI does not visibly escape that part of the reason. Suggested bucket: other,
misleading diagnostic spelling and violation of the existing presentation contract. Confidence: confirmed conditional
source/contract route, not runtime-observed frequency. Could-be-highest:no established security, unauthorized execution,
wrong target, destructive-consent, data/process/work-loss or HTML-execution route. Actual routing retains the correct
raw alias; only its display is wrong. proposed_drop:false for this narrow current defect. Original proposed
source-refutation of arbitrary remote-error/security hypotheses remains recorded below and is not an accepted parent
drop/lowering. possible_cover:g059/l5 literal recipe-reference/account summaries and parent-supplied g086/l4 host-alias
footer share the policy but are distinct producers/sinks/fixes; neither is an alias for this record. No coverage-source
search or general escaping sweep performed. Contract: peer.rs:4-14 defines the boundary as AUTHORSHIP, not distance, and
explicitly includes transport errors, remediations and SSH destinations even when local. :49-55 applies final
presentation helpers to directly decoded proto and plain diagnostic snapshots. display_peer :159-180 makes unsafe
presentation characters visible. Thus the former statement that final escaping here was only future hardening is
superseded for this established configured-alias route. Registration/validity: hosts.rs:1010-1050 passes authenticated
POST /api/hosts spec.ssh unchanged to add_ssh_host; store.rs:2258-2263,6867-6880 allows nonempty, non-leading-dash,
NUL-free, non-podman UTF-8 destinations. The ordinary Add-host help at ui hosts.rs:6177 explicitly supports Host aliases
from ssh config. access.rs:82-93,267-282,340-363 and ssh.rs:267-295 use UserConfig, literal argv destination and proper
option termination; they do not replace it with escaped display spelling. This is not an assumption that arbitrary
invalid DNS names reach capacity checks. Primary compatibility evidence: OpenSSH V_9_9_P2 ssh.c:596-606 accepts the
UTF-8 bytes of U+200B in an alias, :1108-1112 validates it, :1155-1170 reads config and substitutes HostName before
address resolution; readconf.c:1697-1735 matches Host patterns. A matching alias entry with an ordinary HostName
therefore supplies a supported real endpoint without resolving the unsafe alias as DNS. Source URLs:
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_9_P2/ssh.c and
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_9_P2/readconf.c . No config was written or executed, and
no assertion is made that every SSH implementation or every malformed target accepts it. Actual typed-error route:
podman_probe.rs:145-164 retains the ordinary shared SSH path and refusal logging. :223-229 constructs
SessionLimitRefusal only for code 255 plus the client's mux-refusal diagnostic. OpenSSH V_9_9_P2 mux.c:1273-1294 emits
that failure text on a failed session-open confirmation
(https://raw.githubusercontent.com/openssh/openssh-portable/V_9_9_P2/mux.c); this is a real mux failure path, not
fabricated remote stderr. SPEC_impl.md:6482-6487 expressly describes full shared masters, noninteractive fallback and
hosts requiring interactive approval, with a session-limit diagnosis when fallback fails. transport.rs:422-442 formats
the locally written diagnosis with the bounded original destination; manager.rs:5310-5320 truncates without escaping and
its docs defer escaping to the complete failure's display boundary. podman_probe.rs:516-540 preserves the typed error
instead of replacing it with fixed text; :509-513 stringifies it, :446-472 caches the raw reason and :303-316 returns
it. Podman places at podman.rs:952-974 forwards that view.reason unchanged; hosts.rs:4294-4339 sends it as
EnvironmentPlaceView.reason and :4347-4374 supplies it as RecipePlaceView.reason when no build/freshness reason wins.
Exact raw sinks: containers_settings.rs:230-242 concatenates row.reason into the recipe status beside a separately
escaped host name; environment_ui.rs:637-649 appends place.reason to the native Container host option label without
display_peer. A saved Podman-compatible recipe with no competing build failure/freshness reason exposes the first route;
a current/restored container placement or the authoring picker exposes the second. The trigger is the supported
unsafe-character alias plus the actual typed shared-channel/fallback failure, not every request failure. A separately
edited Farhelm display alias is not required and is not confused with the SSH config alias used for routing. Original
hypotheses/refutations: Podman stored build failures pass shell::shown (podman/build.rs:279-286; shell.rs:194-212);
ordinary probe command errors become fixed text except this typed refusal (podman_probe.rs:509-543); validate_info
:667-693 validates semver before quoting it. These still refute arbitrary remote provider-output formatting as the
demonstrated input. Dioxus text interpolation is markup-inert. No route from this diagnostic to an unwarned
destructive/security decision has been established. The new broader contract evidence establishes a local-relayed-text
display defect independently of those original refutations; their bases remain intact. Attribution/persistence: both
sinks are in this group's new files added at 350a4b33, so this route is within the review range. Introducing-to-tip diff
is empty for containers_settings.rs and transport.rs; podman_probe.rs changes only comments, while the assigned later
environment_ui patches add unrelated provider methods and leave this raw option reason unchanged. Fix direction: apply
the final presentation helper to relayed reasons at these two sinks while retaining original bytes for
registry/request/SSH values; do not blanket-normalize the operational destination or invent a remote attacker premise.
All readily checkable input, registration, compatible alias, real typed refusal, projection, sink and contract premises
resolved. No remaining source lead under f4. No runtime tests, builds, reproductions, installs, live/process actions,
checkout edits or nested agents. The first attempted append of the preceding revision was rejected for literal newlines
in its event JSON; it was corrected with newline-to-space conversion and successfully appended through the same evidence
record launcher. No failed invocation was treated as coverage.

Returned proposal (claims-050, event 7): **refuted**, suggested bucket **other**.

Raw reason interpolation exists, but the inspected current Podman producers do not supply the hypothesized unsafe remote
formatting. This does not justify weakening final escaping or claim all future producers are safe.

Original stronger consequence: Invisible or bidi formatting in provider errors might spoof diagnostics; a potentially
highest consequence was conditional on a destructive consent or security decision depending on that spoofing. No such
decision route or HTML execution was claimed or established.

Unresolved premise or limitation: A shipped remote-controlled unsafe reason that reaches these particular unsanitized
labels, and any route from spoofed text to security or destructive consent, remain unestablished. The bounded locally
configured destination is a separate retained limitation.

Proposed basis: Propose source refutation of the inspected remote-output premise only; preserve the original conditional
security concern and the locally configured destination caveat for independent checking.

Returned proposal (claims-134, event 25): **retain-verified**, suggested bucket **other**.

The narrow claim is established by conditional source dataflow and the explicit authorship contract. A supported
configured SSH alias can retain U+200B through a real mux-channel refusal plus failed fallback login, then appear
unescaped in recipe-place status and the Container host option label. This is misleading diagnostic spelling, with no
established remote-attacker, HTML, unauthorized execution, wrong-target, destructive-consent or user-work-loss route.
The rare-diagnostic filter's consequence limb fits, but its rare-trigger limb is not established: unusual accepted
configuration alone does not show a narrow timing window, a point interruption, or input shipped clients never send.
Retain under the filter's express uncertainty rule.

Original stronger consequence: Original f4 considered invisible/bidi formatting from provider errors misleading users,
with a possible security or destructive consequence only if a separate consent/decision route were established. Such a
route was never established. Line 7 refuted arbitrary remote provider-output propagation through these reasons and
treated the locally configured typed-refusal destination as only future hardening. The latter conclusion was reopened
and superseded by the broader authorship contract and supported SSH-alias path at lines 23-25. The former remote-output
refutations remain valid for their exact producers. Dioxus text rendering supplies no HTML-execution route. Preserve all
of these in the parent's final drop/lowering register; this consultation neither independently authorizes those
exclusions nor accepts a parent drop.

Unresolved premise or limitation: No remaining source premise for the narrow conditional diagnostic defect. No runtime
execution, frequency measurement, installed-SSH compatibility test or precise browser visual assessment was performed.
Moving the consequence to highest would require an established route from these exact diagnostic surfaces to
unauthorized execution, secret access, wrong-target or destructive authorization, or user data/process/work loss. The
current routing/request values stay correct. Applying the rare-diagnostic filter would require evidence that its
rare-trigger criterion covers this route; unusual accepted configuration alone is insufficient.

Proposed basis: Retain a new verified other item for final escaping of relayed typed-refusal reasons at the two exact
sinks, preserving raw registry/request/SSH bytes. No exact current coverage or alias found. g059/l5
recipe-reference/account summaries and g086/l4 host-alias footer share a policy but are distinct sources/sinks/fixes; no
deduplication proposed. Do not filter on inferred rarity, and do not claim a security/trust-boundary effect solely
because an escaping helper is a defense elsewhere. Send the original remote/security hypotheses and the narrowly stated
refutation/lowering bases to the separate fresh independent drop gate.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: source-refuted-unsafe-remote-reason-producer-only. Full050 plus parent actual SPEC4935-4953,
transport410-448 and full own f4 history: host-supplied text rule applies, but current job reasons pass shell::shown and
probe raw command/parser failures become fixed prose, semantic version validated. Typed exception preserves bounded
locally configured SSH destination, not remote stderr; this is a real unescaped local-text limitation, not proof of
globally safe display, and no destructive/security/HTML sink is established. Proposed only refutation of original unsafe
REMOTE producer into these labels. Keep local destination/final-escape expectation caveat, no request to weaken
rendering and no unrelated086 raw consumer refutation.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g077 record](records/review-through-00de0b6c/groups/g077.json). Record ID: `f4`.
