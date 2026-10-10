# Build mismatch turns unattended mounts into displacing attaches

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Build mismatch can turn automatic recovery into an unsolicited takeover.

## Details

F99 — **definite** — `crates/farhelm-ui/assets/terminal.js:1448` — Build mismatch turns unattended mounts into
displacing attaches

The terminal mount decision uses one boolean for both permitted unattended attachment and intentional displacement.
Under build mismatch, a successful detail refresh can discover a new tab while unattended attachment is forbidden.
Without a deliberate takeover request, that tab then takes the displacing route and removes the active viewer's control.
Represent forbidden unattended mounts separately from intentional takeover and defer those mounts under mismatch.

## Evidence and triage context

- crates/farhelm-ui/src/api.rs:1249 latches build skew while line 1276 still returns a successful response;
  session_view.rs:942 commits its fetched detail.
- crates/farhelm-ui/src/skew.rs:119 revokes capability; reconnect.rs:209 emits auto=false; session_view.rs:1814
  synchronizes the new specifications and policy.
- crates/farhelm-ui/assets/terminal.js:1448 converts revoked capability into false, line 2993 still mounts, and line
  3756 interprets false as the ordinary displacing path.
- crates/farhelm-supervisor/src/service/handlers.rs:1424 refuses competing ownership only for if_unowned; line 1437
  extracts displaced attachments otherwise.
- crates/farhelm-ui/src/list/view.rs:2544–2546 calls on_renamed after successful rename;
  crates/farhelm-ui/src/lib.rs:1603–1610 updates the selected session without reopening it.
- crates/farhelm-ui/src/session_view.rs:1093–1110 observes that selection update and requests Trigger::Explicit.
  crates/farhelm-ui/src/reader.rs:476–483 permits explicit demand despite build-skew withdrawal; :656–659 performs the
  read.
- crates/farhelm-ui/src/session_view.rs:890–903 commits the refreshed tabs; :1668–1747 constructs terminal
  specifications using the existing lease; :1798–1823 calls farhelmTerm.sync.
- crates/farhelm-ui/src/skew.rs:119–120 makes helm_is_current false after mismatch.
  crates/farhelm-ui/src/reconnect.rs:209–214 passes that value as auto; session_view.rs:1779–1780 supplies the policy.
- crates/farhelm-ui/assets/terminal.js:1447–1448 returns false when capability is manual-only. :2951–2993 still mounts a
  newly discovered tab when no takeover latch exists. :3232–3261 forwards the false value, and :3756–3760 opens
  spec.path instead of spec.pathUnowned.
- crates/farhelm-helm/src/terminal.rs:332–339 selects if_unowned=false for the ordinary route; :195–202 chooses ordinary
  attachment. crates/farhelm-supervisor/src/service/handlers.rs:1424–1439 consequently skips refusal and extracts
  displaced attachments; :1454–1464 shuts down their forwarders with takeover notices. service/terminals.rs:580–602
  applies this to every attachment in the same session with a different lease.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- SPEC.md:1248 and :2239 retain non-displacing recovery and takeover rules. TRIAGE_OUTCOMES.md:5429 and :5456 record
  completed fixes requiring this behavior, not acceptance of the remaining capability case. The open header-action item
  concerns listing refresh only.
- {"basis": "TRIAGE_OUTCOMES.md:5429–5473, takeover-latch-misses-attaching-tabs.md and
  new-tab-mount-displaces-owner-during-recovery.md", "comparison": "The consequence overlaps: a newly discovered tab
  evicts another viewer. The completed decisions address attachment races and discovery during recovery, but do not
  discuss the surviving build-mismatch branch that disables the non-displacing flag. Their completion criteria require
  the opposite behavior. Retain this as a residual defect associated with those fixes."}
- {"basis": "SPEC.md:2233–2240, One GUI at a time", "comparison": "Although several GUIs are generally best effort, the
  text explicitly preserves takeover and displaced-client rules. It does not cover unsolicited control transfer."}
- {"basis": "review_feedback_queue/header-actions-skip-listing-read.md:16–26", "comparison": "That item concerns missing
  sidebar refresh after header Replace/Restart under mismatch. This finding concerns an explicit detail refresh after
  rename causing terminal ownership transfer; trigger, consequence, and affected mechanism differ."}

Caveats:

- The timing sequence was not reproduced.
- Requires a newly discovered terminal, revoked automatic capability, and no takeover latch already preventing the
  mount.
- No process destruction or cross-session input delivery is established.
- Requires a new mount after opening, revoked capability, and no existing takeover latch.
- No end-to-end timing reproduction.
- No process destruction or wrong-session keystroke delivery established.
- Confirmed by source inspection, without runtime reproduction.
- Requires another client to own the session, a newly discovered tab, build mismatch, and no received takeover notice in
  the old view.
- No process loss, data loss, credential exposure, or authorization-boundary bypass is established; classification is
  correctness despite the security report provenance.
- The trigger does not require a person to win a sub-second race, so FILTER.md:116–135 does not justify rejection.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_02_cor:p1:F3`, `ui_desktop_02_sec:p2:F1`.

- `ui_desktop_02_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: high.
- `ui_desktop_02_sec:p2:F1`: confidence as filed: definite; suggested bucket as filed: high.
