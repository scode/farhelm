# Empty filter padding overridden by status rule

Reviewed commit: a657a1aa9e26c103704196819f2d6056dad655dc

## TLDR

A generic style overrides the intended spacing between an empty-filter explanation and its clear-filter links.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/assets/app.css:6375 at a657a1aa9e26c103704196819f2d6056dad655dc`.

Source assessment (read with the later corrections below) (review record g015/l6, event 16):

Reviewed commit: a657a1aa9e26c103704196819f2d6056dad655dc. TLDR: the links that clear an empty session filter sit
farther from their explanation than the new layout requests. Details: .filter-empty at app.css:6375-6378 sets
padding-bottom:8px, but the later equal-specificity .status at lines 8802-8805 sets padding:2em and font-size:14px.
view.rs:5060 renders the same element with class=status filter-empty. The later shorthand therefore overrides the bottom
padding to 28px; .filter-empty-actions at line 6392 adds no compensating top padding. The assigned landing report at
plans/reports/host-filter-feedback.report.md:111-113 records the same 28-versus-8 difference. Make the specialized
selector .status.filter-empty, move the override after .status, or otherwise define the intended cascade. Suggested
bucket other (minor presentation defect), confidence confirmed by CSS cascade; visual dimensions beyond this specified
14px font are not measured. proposed_drop:false. possible_cover: host-filter presentation interface; parent decides
coverage. Caveats: no controls become unavailable on the demonstrated route, and SPEC.md does not promise a pixel
spacing value. This is a local rule defeated by a later generic rule, not a claimed broader usability failure. No later
app.css change occurs before pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g015/l6`; supporting records
`g090/d6`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
