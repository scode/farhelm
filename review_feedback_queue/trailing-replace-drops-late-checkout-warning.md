# Timed-out Replace can lose its checkout warning

Reviewed commit: df6199142d6495a95b2d008400ba2686efb775c6

## TLDR

A replacement session can start while the warning explaining where the original checkout was left disappears after the
reply deadline.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/sessions.rs:5148`.

the normal POST /api/sessions/{id}/replace registration is lib.rs:568–572. sessions.rs:4870–4892 supports a different
existing folder through the ordinary create-then-remove lane. Both that lane (:4936) and fresh-checkout Replace (:5011)
call finish_replacement, which captures outcome.notice (:5049–5056) and returns it (:5117–5122). The route sends it only
in the success reply (:5148–5191). The bounded waiter returns 504 (:3830–3839) and log_late_failure drops the eventual
successful body (:3852–3862). GUI session_list/replace.rs:391–415 publishes the band only for a decoded Outcome::Done
with delete_notice. No late-notice persistence is wired into Replace. SPEC.md:1725–1732 and :2012–2018 require the
archive outcome to be visible, and SPEC_impl.md:3759–3764 explicitly includes Replace's source Delete. Plain same-folder
Replace normally retains the reference, so the ordinary existing-folder route is affected when it uses a different
folder. Files are retained per the archive outcome, and supervisor teardown.rs:768–799 independently logs path, reason
and whereabouts. Bucket: high for loss of the user-facing recovery information. Confidence: confirmed by pinned source.
Possible coverage: sessions_tests.rs:3897–3960 verifies a timely success body.
a_late_delete_keeps_its_checkout_notice_in_the_feed only exercises DELETE. No existing assertion verifies delivery of
the notice after a successful late Replace.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `notices/l7`.
