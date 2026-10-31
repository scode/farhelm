# Settings guide misstates device-local preferences

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The guide singles out appearance as device-local even though Sounds is device-local too.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The guide's comparison with other settings conflicts with the inspected preference ownership. This is a
source/documentation mismatch without a cross-device mutation or privacy consequence.

Source locations: `website/src/content/docs/docs/using/manage-hosts.mdx:152-157`. Historical revision qualifiers in
those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Confirmed documentation inconsistency at reviewed tip d6645f771bee8ed03a66486ef8755834937d759d; introduced by assigned
commit 3d10c30f027925e3cec57f7439395ebd8fece7cf. TLDR: the guide tells readers that the other Settings choices belong to
their helm, so they can expect notification-sound choices to follow them to another device when they do not. The added
sentence at manage-hosts.mdx:152-154 says 'Unlike the other settings' the appearance choice is stored on this device;
Notifications follows immediately at 154-155. But SoundSettings at crates/farhelm-ui/src/sounds.rs:196-224 explicitly
says the controls apply only to this device and sends no preference writes; assets/sounds.js:104-109,123-134
reads/writes localStorage. The linked session-list.mdx:124-133 correctly states sounds stay on this device.
SPEC.md:1672-1680 likewise distinguishes app-installation updates and device-local sound switches from shared helm
preferences. This is a narrow explanatory wording defect, not an implementation persistence bug. Suggested bucket other.
Confidence confirmed by source, no runtime check. proposed_drop: false. possible_cover: none found. Targeted searches
across SPEC.md, SPEC_impl.md, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, review_feedback_queue/INDEX.md and plans/queue found
device-local sound contracts, test-cleanup outcomes and the settings-save plan's exclusion, not acceptance or repair of
this false documentation comparison. TODO Planned is empty at 23-25. Not potentially highest: the effect is a mistaken
preference-synchronization expectation; no lost work or security route is alleged.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g088/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
