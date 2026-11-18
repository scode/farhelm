# Stop and suspend hides its timeout explanation

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

When Stop and suspend exceeds its reply deadline, Farhelm can leave you without the explanation that it is still working
or that Stop already failed.

## Details

Haiku pass H-P6-3, checked by gpt-6.2-sol. Suggested queue bucket **other**. Confirmed dropped outer timeout
explanation. The consolidated P6 pass also identified a completed Stop failure returning the same status.

- Verdict: NOT CRITICAL
- Route checked: `session_list/environment.rs:624–659`, `farhelm-helm/src/sessions/sandbox.rs:535–599`,
  `SPEC.md:1588–1591`, and `SPEC_impl.md:259–269`.
- Reason: The native UI drops the 504 explanation required by the product text. That 504 ends the wait while the helm's
  owned operation carries on. The Stop must finish cleanly before Suspend, and live state comes from the session and
  host feeds. No success is recorded from the 504. The missing explanation can make the action appear to have gone
  nowhere.

Show the carries-on explanation and preserve the terminal Stop refusal. Give the continuing response a typed marker so
the GUI can distinguish the two outcomes. Check the row after both kinds of timeout.

SPEC.md:1588–1591 and SPEC_impl.md:259–269 specify the continuing receipt. The status-and-history filter covers narrower
inaccurate session history after a partly failed operation. This route also drops an expressly required response while
the helm still owns work, and it can occur during an ordinary slow operation.

### A completed Stop can also reply 504

The Haiku consolidation's P6 check found that the same branch drops a Stop failure with status 504 when Stop ends before
the helm's outer wait finishes. Source confirmed at
`crates/gui/farhelm-gui/src/session_list/environment.rs:111–117,644–659`,
`crates/farhelm-helm/src/sessions/sandbox.rs:535–549`, and `crates/farhelm-helm/src/sessions.rs:3774–3791`. The 504
status alone cannot distinguish a completed Stop error from a continuing operation. A typed marker would preserve the
completed refusal and let the GUI show the continuing receipt's separate explanation. Suggested bucket **other**.
