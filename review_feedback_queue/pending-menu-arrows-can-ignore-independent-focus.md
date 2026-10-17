# Pending menu navigation can still override independent focus

Reviewed commit: 2c2fc63ab9bfc9677e5ad394023d9d3a22b87422

## TLDR

Moving focus with a pointer or screen reader while menu arrow presses are still landing can make the next arrow choose
an unexpected command, potentially Delete. Activating it still requires Enter or Space. The narrower fix was explicitly
accepted in its historical plan; that acceptance is missing from the specification and triage ledger.

## Details

Suggested bucket: conservatively `highest`, conditional on unexpected navigation followed by activation deleting
user-owned work. Confidence: likely; the request-precedence mechanism is confirmed statically, but the independent focus
event sequence and destructive outcome were not reproduced. Provenance: separate adversarial reviewer in the trailing
review through afc828776596f2e6639d16d4203ccc8b3d0f5862, followed by an independent exclusion audit. Proposed drop:
accepted residual behavior; the independent reviewer could not agree unambiguously under the queue's named
coverage-source rule, so this record preserves that ambiguity.

At the reviewed commit, `crates/farhelm-ui/src/menu_panel.rs:562–564` retains request precedence. Its focus handler
clears the request only when the requested position receives focus (`:573–575`). Independent focus elsewhere while a
request is pending leaves it intact (`:582–596`), so the next arrow still steps from the old request (`:647–650`). The
change repairs independent focus moves after navigation settles, while deliberately retaining the pending window. The
removed `menu-request-overrides-independent-focus.md` finding described the conditional path to immediate deletion of an
ended session without tabs. This record narrows that finding to the remaining pending window.

Possible cover: substantial, but not in the sources the recording procedure enumerates. At commit
b00f8402d541ff68c86868387d02deff1fb12a73, `plans/queue/menu-arrows-follow-focus.md:260–312` records the blocked
question, including the unexpected-Delete consequence and need for subsequent activation, and the maintainer's answer
“(1)” accepting the narrower fix. Its agreed restatement explicitly accepts the pending window for that plan and tracks
a broader design separately. Current TODO.md's Near term entry “Consider letting menu arrows follow focus moved
mid-navigation” repeats the residual behavior. Near term is not Planned.

The independent auditor found no matching acceptance in SPEC.md, SPEC_impl.md, Planned TODO items, BUGS.md, or the
triage ledger. `TRIAGE_OUTCOMES.md`, “menu-request-overrides-independent-focus.md”, still records broader completion
criteria, while SPEC_impl.md's menu keyboard contract requires ordinary arrow navigation. This is an acceptance-record
gap, not a claim that the implementation ignored the maintainer's decision. Triage should reconcile that explicit
decision before commissioning a broader focus-source design.

No runtime tests or assistive-technology reproduction were run. The historical plan's full question and decision were
read. The independent auditor agreed with the separate refutation of stale focus landing at a repeated position; that
does not refute this independent-focus case.
