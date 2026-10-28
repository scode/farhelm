# Hidden-progress test observes a host outside the affected set

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

The new regression for hidden host management can miss removal of its movement guard: the menu it observes is already
protected by a separate row-order rule.

## Details

Suggested bucket: **other**, for test coverage. Confidence: **confirmed source-level oracle gap**. No production menu
failure or whole-suite mutation result is claimed.

`e2e/tests/provisioning.spec.ts:2100–2112` opens the existing local host's rail menu while the newly added remote host
completes. The manager explicitly orders local first (`farhelm-helm/src/manager.rs:1873–1904`); host views and GUI
rendering preserve that order (`src/hosts.rs:408–450`; `farhelm-ui/src/hosts.rs:2354–2362,2847–2855`). Provisioning
movement selects only hosts below the changing row (`farhelm-ui/src/hosts.rs:1500–1542`; `provisioning.rs:2828–2841`).
Removing the concealment guard at `hosts.rs:1685–1693` therefore still leaves this local menu unaffected. Its connected
state establishes liveness, rather than the causal order needed for the negative assertion. The omission remains at
pinned tip `9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

Use a bystander after the changing host, assert their displayed order, and observe that bystander's rail menu. A
comparable visible-management positive control would distinguish concealment from the independent ordering rule. No
source premise remains missing; no runtime mutation was performed.

SPEC.md:1483–1493 and SPEC_impl.md:670–679 require concealment to preserve menus. The baseline-flake queue record names
historical failures of this same test, with a different cause. The landed sidebar plan is not a pending repair. No exact
Planned, unlanded-plan, BUGS, queue, ledger or filter coverage was found. Possible cover: none. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g017/g017-f3`, found during synthesis. The
separate allegation that the menu needed measured placement was withdrawn after checking synchronous registration; this
finding concerns which registered menu the test chooses.
