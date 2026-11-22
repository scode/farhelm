# Failed-replacement Restart can restore the previous run while a recorded scope remains unconfirmed

Reviewed commit: cfcc29bf0794c06ac876d2fe665bdbefe7d602f2

## TLDR

After a failed replacement, Restart can restore the previous run and show only the launch error while cleanup of the
replacement scope is unconfirmed.

## Details

**other.** Source inspection.

SPEC_impl.md:2087–2100,2124–2126 and crates/farhelm-supervisor/src/service/core/host_restart.rs:745–782,
cfcc29bf0794c06ac876d2fe665bdbefe7d602f2; crates/farhelm-supervisor/src/service/sweep.rs:1567–1576,1613–1632 and
service/core.rs:12929–12969,13059–13076, 1a53d9d35889d7dde96e31320cc1681dd013737d

Scope recheck: the assigned patch changes only the Restart contract here. It replaces the sole trigger of an unconfirmed
replacement-host end with two triggers, host end or unconfirmed process-tree sweep, and says either leaves the new
generation launching. It also expands the residual-history paragraph to cover unconfirmed process-tree cleanup. TLDR:
When the recorded scope is unconfirmed but the portable sweep succeeds, Warn converts the scope failure to a log
warning; unwind returns definitive and the caller attempts to restore the prior run. This code predates the patch and
remains at the pinned tip. The user sees a failed Restart without the scope uncertainty. SPEC.md:2263–2270 at the pinned
tip requires confirming every recorded mechanism. SPEC_impl.md:3024–3034 grants the weaker proof for ordinary keyed
Create and 2089–2102 cross-refers to Create while distinguishing unconfirmed Restart cleanup. SPEC.md:2275–2289 bounds
the coinciding rare-fault exception. No surviving process or work loss is established. The reviewer suggested other,
confidence confirmed for the code route. Possible coverage: only if the failed-create cross-reference is intended to
extend that exception to Restart. Proposed_drop no.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `implementation-contracts/IC-10`.
