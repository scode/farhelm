# An ordinary host update disables Settings for every host

Reviewed commit: e58aafe5a8afe97d7bc7672fe4519eddcf4044e2

## TLDR

Updating one remote host prevents opening Settings on unrelated hosts, including the local host, until that update
finishes.

## Details

Suggested bucket: `other`. Confidence: confirmed by source inspection; no browser reproduction. Source: dedicated
`gpt-6-astra` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/hosts.rs:1737 and :1782–1785 now treat any ProvisioningMenuState.planning flag as a page-wide
Settings prohibition. That flag does not mean only a pending modal-producing plan: provisioning.rs:90–94 explicitly
includes accepted/observed running updates; :714–715 retains ownership throughout the run; :2488–2508 publishes it as
planning. Consequently an ordinary slow upload or installation disables every Settings entry through hosts.rs:3403 and
:3418. Update produces no confirmation modal (SPEC.md:104–106), so this blocks unrelated host configuration without
providing modal protection. Publish/check a separate pending-confirmation condition rather than using the broader
provisioning-busy summary.

Possible cover: TRIAGE_OUTCOMES.md:12621–12632 authorizes blocking Settings while an uninstall confirmation is pending,
not throughout unrelated automatic updates. Heading newer-completed-run-leaves-older-update-permanently.md accepts a
different multi-GUI race affecting the tracked host; this regression requires one GUI and an ordinary update.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: hosts.rs:1767 and :1812–1815 retain the
global guard; provisioning.rs:2498–2518 retains the broad planning calculation. No fixing commit found.

Review coverage: full commit diff read: true. No part of the reviewed commit diff remained unread. Unchanged surrounding
modules, most unrelated specification/ledger sections, and unrelated queue finding bodies were not exhaustively read.
Some aggregate outputs truncated search/context material; relevant missing material was reopened in smaller reads. The
unrelated remainder of the later settings-dialog diff was not pursued. No Sol reports or working-copy queue additions
were read.

Runtime evidence: None, as requested by default. Applied the supplied test-authoring checklist to every changed
test/fixture by inspection. Findings are source-confirmed, without browser reproduction. No checkout files were created,
modified or deleted; no fetch, rebase or nested agents.
