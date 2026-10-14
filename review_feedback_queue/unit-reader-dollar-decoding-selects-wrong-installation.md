# Dollar decoding can misidentify service ownership

Independent drop decision: a fresh `gpt-6.1-sol` high reviewer rejected the proposed exclusion. The ledger's completion
criteria prevent rendering dollar-bearing paths; they do not clearly accept continued wrong ownership attribution when
reading existing services. The specifications require unsupported installations to be refused. Retain this ambiguity for
human triage. The two historical reports below describe one root cause and count as one distinct new finding.

Reviewed commit: b226160d78884053136a35120b508231821e7cfe

## TLDR

An edited service containing literal dollars can be attributed to another installation during uninstall.

## Details

Suggested bucket: `highest`. Confidence: confirmed parser mismatch; ordinary occurrence and process loss not reproduced.
Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: Covered by TRIAGE_OUTCOMES.md heading
executable-read-back-collapses-dollars-select-another-installations.md, lines 8371-8391. Retained here for mandatory
independent drop review; no independent reviewer was spawned..

crates/farhelm-helm/src/units.rs:304-306 collapses
$$ to $, although systemd preserves dollars in the executable position. A marked unit naming /opt/a$$b/farhelm can
therefore match selected /opt/a$b/farhelm. crates/farhelm/src/setup.rs:501-517 classifies that path; :898-914 stops the
service and removes its file. Requires matching installation paths and an edited marked unit. This behavior predates the
reviewed commit.

Possible cover: The ledger explicitly addresses this mismatch and chose refusal of dollar-containing installation paths.
Reviewed SPEC.md:2511-2513 declares those paths unsupported. Deliberately fabricated filesystem interference is
separately excluded by SPEC.md:2256-2260.

Status at frozen endpoint: The decoder still exists at pinned main units.rs:347-349. Commit
3502b69062a39c25dae11e52a1747e9137b50067 prevents newly rendered services from containing dollar paths; it does not
remove the edited-unit readback mismatch.

Review coverage: full commit diff read: true. No commit-diff content unread. Broad search output and one combined
contextual read were truncated; relevant code and specification portions were reopened in bounded reads. The entire
13,089-line current triage ledger and entire specifications were not read; relevant sections and heading searches were
read. Unrelated later procs.rs changes beyond the first 190 diff lines were not reviewed. A guessed historical server.rs
path did not exist; the actual provisioning caller was located and read. No new actionable regression attributable to
this commit was established; findings retain covered pre-existing highest candidates for independent drop review.

Runtime evidence: None, as requested. No macOS compile, live kernel read, systemd execution or destructive reproduction
performed. Checkout files were not created, modified or deleted. HEAD confirmed as
4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703.

## Additional affected scope

Reviewed commit: 3502b69062a39c25dae11e52a1747e9137b50067

## TLDR

Uninstall can still attribute an existing dollar-bearing service to a different installation.

## Details

Suggested bucket: `highest`. Confidence: likely consequence; parser behavior confirmed, but the two-installation
filesystem premise and destructive action were not reproduced. Source: dedicated `gpt-6.1-sol` high adversarial review.
Proposed drop: Possible exact-item ledger coverage; leave the drop to an independent reviewer. The chosen completion
criteria target rendering refusal, while the specification also says unsupported installations are refused..

crates/farhelm-helm/src/units.rs:347–349 still collapses executable-path
$$ into $, although systemd preserves executable dollars. The new refusal at :414–422 protects rendering, not inspection of existing units. A marked simple service naming /opt/$$name/farhelm
can therefore match a selected /opt/$name/farhelm installation through crates/farhelm/src/setup.rs:501–517, and removal
at :904–925 affects the other service. The post-confirmation recheck in crates/farhelm/src/uninstall.rs:299–308 repeats
the same parser. Requires distinct dollar spellings resolving to different installations and recognized flat-install
ownership. This mechanism predates the commit; its feedback file is deleted here.

Possible cover: TRIAGE_OUTCOMES.md at reviewed commit:8362–8382, heading
executable-read-back-collapses-dollars-select-another-installations.md; current ledger:8371–8391. SPEC.md:2509–2513
declares dollar-bearing program paths unsupported and refused. Neither explicitly says inspection may misidentify an
existing installation.

Status at frozen endpoint: Parser mechanism persists at pinned main units.rs:347–349 and setup.rs:501–517. New unit
generation is prevented by the reviewed commit, but no later fixing commit changes the reader.

Review coverage: full commit diff read: true. No commit-diff hunks unread. Initial combined-output truncations were
followed by complete targeted rereads of the affected diffs and relevant specification content. Unchanged modules and
the remainder of both specifications and the large triage ledger were not exhaustively read; ledger searches targeted
this commit's mechanisms. Independent highest-drop review remains for the parent; no nested agents were launched.

Runtime evidence: None, as requested. Static historical/current-main tracing only; supplied test-authoring checklist
applied to all changed tests and relevant helpers. No checkout files or scratch artifacts created, modified, or deleted.
