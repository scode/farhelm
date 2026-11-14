# Container placement footer renders host formatting characters verbatim

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: A container host name containing invisible or bidirectional formatting is shown visibly
escaped in the placement picker but rendered verbatim in the launcher's footer, so the same selected host can have a
different or misleading visible spelling beside Launch. Source route: helm hosts.rs:4317-4328 populates
EnvironmentPlaceView.name from aggregate::host_display_name; aggregate.rs:119-125 returns an alias raw.
store.rs:2014-2032 validates aliases only by trim, char::is_control and 64-character cap, which does not exclude
formatting characters such as U+202E or U+200B. These are explicitly classified unsafe by proto text.rs:57-78. GUI alias
drafting uses the same control-only policy (hosts.rs:3772 onward), so this is an admitted name shape, not a forged
response. environment_ui::target_label at :302-315 combines raw recipe and place.name and documents that rendering owns
escaping. The assigned new branch at create_form.rs:4395-4401 uses that result without display_peer; :4419-4423 applies
display_peer only for Restart mode; ordinary container New/Clone therefore renders the raw label at :9188, with the same
value also entering target summary/managed-checkout description. By contrast the ordinary host path uses
HostOption::label (:4405), whose gui_host_name goes through display_peer (shared.rs:413-431; hosts.rs:318-323), the
shared PlacementPicker displays place.name through display_peer (environment_ui.rs:642-648), and recent_placement_label
wraps target_label in display_peer (launch_composer.rs:1602-1613). peer.rs:159-169 is the missing escaping boundary.
Contract: SPEC.md L

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g086/l4`.

### g086/l4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 20 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. TLDR: A container host name containing invisible or
bidirectional formatting is shown visibly escaped in the placement picker but rendered verbatim in the launcher's
footer, so the same selected host can have a different or misleading visible spelling beside Launch. Source route: helm
hosts.rs:4317-4328 populates EnvironmentPlaceView.name from aggregate::host_display_name; aggregate.rs:119-125 returns
an alias raw. store.rs:2014-2032 validates aliases only by trim, char::is_control and 64-character cap, which does not
exclude formatting characters such as U+202E or U+200B. These are explicitly classified unsafe by proto text.rs:57-78.
GUI alias drafting uses the same control-only policy (hosts.rs:3772 onward), so this is an admitted name shape, not a
forged response. environment_ui::target_label at :302-315 combines raw recipe and place.name and documents that
rendering owns escaping. The assigned new branch at create_form.rs:4395-4401 uses that result without display_peer;
:4419-4423 applies display_peer only for Restart mode; ordinary container New/Clone therefore renders the raw label at
:9188, with the same value also entering target summary/managed-checkout description. By contrast the ordinary host path
uses HostOption::label (:4405), whose gui_host_name goes through display_peer (shared.rs:413-431; hosts.rs:318-323), the
shared PlacementPicker displays place.name through display_peer (environment_ui.rs:642-648), and recent_placement_label
wraps target_label in display_peer (launch_composer.rs:1602-1613). peer.rs:159-169 is the missing escaping boundary.
Contract: SPEC.md Local authority and trust between hosts says ‘text a host supplied that the GUI shows outside a
terminal is escaped so invisible and direction-changing characters are visible, including in accessible names, wherever
it appears and however it reached the helm’. Agent-authored aliases also pass store::validate_alias
(agent_requests.rs:1136, :3137); the name remains raw fleet data, not display-safe text. Correct rendering does not
require rejecting Unicode aliases: escape the newly composed selected-host display label, retaining raw target identity
for requests. Suggested bucket: other (presentation/formatting correctness; no established wrong-target or unauthorized
action route). Confidence: confirmed conditional source route, not runtime observed. proposed_drop: false.
possible_cover: none matching the supplied interface findings. Could-be-highest: no established security/loss-of-work
route: the picker itself escapes the host label, Dioxus text interpolation does not interpret HTML, and the request uses
the correct captured host id. Do not infer wrong-host execution from this display omission. Attribution: the exclusive
base ordinary host branch already uses HostOption::label and has no container-place target label; raw sandbox recipe
text also existed before the assignment and is not a new independent claim. This finding is specifically the assigned
350 commit's newly integrated container-host-name label path. End revision 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db
retains the same unescaped branch at create_form.rs:4395-4423, so no later repair was found. All readily checkable
input/projection/rendering premises settled; l4 is resolved from lead. No reproduction/test/build/live-install action
performed.

Returned proposal (claims-056, event 20): **retain-verified**, suggested bucket **other**.

The container placement label admits an alias containing format characters, composes it as raw text and reaches the
ordinary New/Clone footer without display_peer. This violates the visible-formatting rule. Correct captured host
identity and text interpolation do not establish wrong-host execution or HTML injection.

Original stronger consequence: The same selected host can have a misleading or different visible spelling in the footer
beside Launch versus its escaped picker label. No wrong-target action, unauthorized execution or HTML interpretation is
claimed.

Unresolved premise or limitation: A stronger security/work-loss consequence would need an actual harmful
target-confusion route; none was established and requests retain the correct host id.

Proposed basis: Retain the conditional source-confirmed escaping omission as presentation correctness; do not promote it
to a security incident without the missing route.

Existing parent disposition (event 20): verified. Parent explicitly accepted named retain-verified proposal from
claims-056 after inspecting it. The container placement label admits an alias containing format characters, composes it
as raw text and reaches the ordinary New/Clone footer without display_peer. This violates the visible-formatting rule.
Correct captured host identity and text interpolation do not establish wrong-host execution or HTML injection. Retain
the conditional source-confirmed escaping omission as presentation correctness; do not promote it to a security incident
without the missing route. Checked source/results: ["350a4b33:crates/farhelm-helm/src/store.rs:2014-2032 rejects
char::is_control but not Unicode format characters; hosts.rs:4317-4330 projects the raw alias through host_display_name
into EnvironmentPlaceView.name.","350a4b33:crates/farhelm-ui/src/environment_ui.rs:300-317 returns raw place.name in
target_label and assigns escaping to rendering. create_form.rs:4395-4423 escapes only Restart's label, leaving New/Clone
raw.","350a4b33:create_form.rs:9188 interpolates selected_host_label directly into the footer's peer-value span; dir=ltr
does not call format-character escaping. 00de0b6c:create_form.rs:4395-4423 retains the raw environment-label branch.
This is the newly integrated container-host-name path; raw sandbox recipe text predates it. No runtime rendering or
harmful reliance demonstrated."] Current coverage/results: ["SPEC.md:4944-4948 expressly requires visible escaping of
invisible and direction-changing host text. SPEC_impl.md peer-text/host-identity rules provide no footer
exception.","Planned empty; BUGS.md unrelated; searched plans/queue and active ledger for footer/container-placement
formatting, no exact accepted omission or planned repair.","Read
review_feedback_queue/launcher-target-hover-merges-direction-runs.md: it concerns grouping ordinary RTL text after
escaping, whereas this claim omits escaping of unsafe format characters.
remote-version-display-retains-format-characters names a different display owner. Neither is exact coverage. FILTER.md's
rare diagnostics rule does not cover an admitted placement display used in ordinary launch."] Exact cited cover: []
Original stronger consequence and missing premise remain preserved: "The same selected host can have a misleading or
different visible spelling in the footer beside Launch versus its escaped picker label. No wrong-target action,
unauthorized execution or HTML interpretation is claimed."; "A stronger security/work-loss consequence would need an
actual harmful target-confusion route; none was established and requests retain the correct host id.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g086 record](records/review-through-00de0b6c/groups/g086.json). Record ID: `l4`.
