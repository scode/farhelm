# Launch approval omits the trusted-workspace status and warning from native accessibility

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Screen-reader users can miss the trusted-workspace status and warning on a launch or Restart approval.

## Details

approvals/model.rs:726–753 and :942–960 build the permission fact from the stored launch, including `workspace_trust`.
approvals/card.rs:395–406, :454 and :576–606 render the true branch as an anonymous chip and raw text. These Div and
string elements carry no IDs or accessibility roles, both required by vendor/gpui-pre/src/element.rs:445–460. The card's
accessible group label supplies the heading only (card.rs:160–163), and its answer buttons carry their own action labels
and help (:723–769). The raw start/resume command is a separate fact in model.rs:750–753. No accessible status or
explanatory sentence duplicates the omitted permission fact. SPEC.md:4412–4419 requires the trust status on the approval
card. The actual approval stays bound to the drawn request (approvals/region.rs:335–340,405–407). The defect is that the
native accessible presentation leaves out one of its permission facts. Expose the fact and its warning as named content
in the card's accessible tree before the answer controls.

Confidence: confirmed by inspection of the emitted native tree and bound approval path. Real screen-reader output was
not exercised.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agUI/l19`.
