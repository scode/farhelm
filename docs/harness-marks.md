# Harness marks in the session sidebar

The session list identifies each agent harness with a small mark beside the permission mark. This document records where
every mark comes from, what the brand owner's terms allow, and the one sizing rule they all obey, so that the next
person to touch `crates/gui/farhelm-gui/src/marks.rs` does not have to redo the research or guess why a given harness
does not carry its official logo. That module holds the geometry as plain data (paths, bounding boxes, transforms) for
the GPUI app to draw; the old Dioxus UI drew the same data as inline SVG. Geometry research date: 2026-09-22; the color
re-check is recorded below. Brand terms change; re-check the cited pages before relying on a statement here for a new
use.

## The sizing rule

Every mark is drawn in a shared 12-unit box, through a transform that applies one rule to the mark's native coordinates:
scale so the longer side of its bounding box spans 10 units, then centre it in the box. With `w`, `h`, `x`, `y` the
native bounding box and `s = 10 / max(w, h)`, the transform is
`translate((12 - w*s)/2 - x*s, (12 - h*s)/2 - y*s) scale(s)`. The bounding box is recorded beside each transform, and a
test checks every transform against the rule. The old UI rendered the box in a 10×12 CSS pixel slot.

The rule exists because the marks are only ever seen next to each other. Before it, three marks were SVG `text` elements
at font-size 9 and drew at roughly three quarters the height of the path marks beside them. Do not render a mark as
text, and do not hand-tune a transform to make a mark "look right": change the bounding box, recompute, and keep the
rule.

Official geometry is scaled by the rule and nothing else; the color treatment recorded below is applied by the view that
draws the mark. Several of the terms below forbid modification, and an unaltered path is also what lets
`THIRD_PARTY_NOTICES.md` cite an upstream file at a commit rather than describe a derivative work.

## Per-harness provenance

| Harness  | Mark                      | Status                 | Source                                                                          |
| -------- | ------------------------- | ---------------------- | ------------------------------------------------------------------------------- |
| Claude   | eight-ray starburst       | Farhelm original       | drawn here; not the Anthropic spark                                             |
| Codex    | OpenAI blossom            | official, unaltered    | `OpenAI-black-monoblossom.svg` from OpenAI's brand download                     |
| Muse     | stroked "M" with dot ends | community, unofficial  | `resources/muse-logo.svg` in `corona10/muse-code-vscode` (MIT), sparkle dropped |
| Cursor   | 2D cube                   | official, unaltered    | `CUBE_2D_LIGHT.svg` from cursor.com/brand                                       |
| Goose    | goose silhouette          | official, unaltered    | `ui/desktop/src/components/icons/Goose.tsx` in `aaif-goose/goose` (Apache-2.0)  |
| Pi       | pixel-grid "P" with dot   | official, unaltered    | `https://pi.dev/favicon.svg`, the press kit's badge mark                        |
| OMP      | block-style π             | official, tile dropped | `https://omp.sh/favicon.svg` path (MIT repo `can1357/oh-my-pi`)                 |
| Grok     | block "G"                 | Farhelm original       | drawn here; retained independently of the current official kit                  |
| OpenCode | even-odd ring             | official, unaltered    | `packages/ui/src/assets/favicon/favicon-v3.svg` in `anomalyco/opencode` (MIT)   |
| Terminal | window with prompt        | Farhelm original       | drawn here                                                                      |

### Claude — Farhelm original, by constraint

Anthropic's trademark guidelines (`https://www.anthropic.com/legal/trademark-guidelines`, effective 2024-08-01) allow
use of its marks "only as specifically permitted by us and only in materials we approve beforehand", state that "no
alterations of our trademarks (changes to color, font, proportion, or otherwise) are permitted", and offer no
compatibility or referential carve-out. The Claude Code legal page
(`https://code.claude.com/docs/en/legal-and-compliance`) narrows this to: you may say in plain text that a product runs
Claude Code, but "any other use of Anthropic's names or logos ... requires our written permission". Anthropic's official
brand skill publishes a palette (linked under Colors); that does not grant permission to copy the spark. Its SVG is easy
to obtain (the VS Code extension ships it, simple-icons carries it), which is not the same as being allowed to show it
recoloured in a third-party sidebar.

So the Claude mark is Farhelm's own drawing: eight tapered rays from a common centre. It is meant to evoke "spark"
without reproducing the spark, whose twelve irregular hand-drawn rays are what make it recognisable. If Anthropic
publishes referential-use terms, revisit; until then, do not swap in the real spark. The maintainer accepted coloring
this original starburst in Claude's terracotta on 2026-11-03, including its closer resemblance to Anthropic's spark. The
geometry remains Farhelm's own.

### Codex — OpenAI blossom, official

Farhelm identifies Codex with the OpenAI blossom. OpenAI's brand page (`https://openai.com/brand/`) is written as a
self-serve licence for third parties: use the logo "only when it directly relates to OpenAI services", "exactly as
provided", not "more prominently than your own", and do not "modify it in any way". Official black and white
"monoblossom" SVGs are in `https://cdn.openai.com/brand/OpenAI-Logos-2025.zip`. The mark is the `d` attribute of
`OpenAI-black-monoblossom.svg` byte for byte, kept in `marks/openai-blossom.svgpath` because a 2.5 KB path does not
belong on one source line and must not be simplified. The permission is revocable; if OpenAI's terms change, the mark
comes out.

At 12px the blossom's interlocking arms lose their gaps and it reads mostly by silhouette. That was judged acceptable
against the alternative of inventing something in its place.

### Muse — community icon

No Muse Code-specific mark was found in its product page or SDK repository on the color re-check. Its product and docs
pages (`https://dev.meta.ai/products/muse-code/`) carry only the Meta corporate lockup and favicon, the official SDK
repository has no image assets, and Meta's brand resources say that "all usage of the Meta logo requires approval" and
that Meta's marks "may only be used as provided in these guidelines or with Meta's permission". The consumer "Muse" app
launched in September 2026 with a scribble-M icon, but it exists only as raster artwork and Meta has not connected it to
Muse Code.

The mark is therefore taken from `corona10/muse-code-vscode`, an unofficial VS Code extension under MIT whose README
states it is not made or endorsed by Meta. Its icon is a stroked "M" with round dots at the four terminals and a small
sparkle; Farhelm keeps the M and the dots, replaces the community blue with a Farhelm hue and drops the sparkle, which
at 12px is a speck. This is an MIT-licensed placeholder, not an official brand: it carries no recognition value beyond
the letter. Replace it if Meta ever publishes a Muse Code mark with terms that allow this use.

### Cursor — official cube

`https://cursor.com/brand` offers logo, cube, wordmark, app icon, and avatar downloads. Its only written guidance is
which variants exist and "Refer to us as Cursor. Not Cursor AI or Cursor Code"; there is no trademark policy page, and
the marketplace publisher terms that reference the brand guidelines bind marketplace publishers, not referential use.
The 2D cube is one path, and cursor.com renders the same geometry inline in `currentColor`. Its downloads and favicons
use neutral light and dark variants, so Farhelm's hue is its own choice, not an official Cursor color. No express color
restriction or recoloring grant was found in the checked brand guidance; the recorded referential-use judgment remains
the basis for this treatment. The 2.5D variant is five grey fills and cannot be rendered in one colour; do not use it.

### Goose — official glyph, permitted but coarse

Goose moved from Block to the Agentic AI Foundation under LF Projects in 2026 (`github.com/block/goose` redirects to
`aaif-goose/goose`). The repository is Apache-2.0 with no logo exclusion; trademark use falls under the LF Projects
policy (`https://lfprojects.org/policies/trademark-policy/`), which permits referential use and asks that a logo not be
"displayed with color variations". Both the pinned and current `Goose.tsx` glyph use `currentColor`. Farhelm keeps its
existing inherited grey rather than adding a hue; this is not a claim to reproduce an exact official black or white.

The glyph is a detailed silhouette whose smallest upstream use is 24px. At 12px it reads as a bird-shaped mark. It was
kept after seeing it at real size, on the grounds that the official shape is preferable to an invented one, and the
`Goose.tsx` 24-unit path is the least noisy cut the project publishes.

### Pi — official badge

pi.dev has a press kit (`https://pi.dev/press-kit`) with a primary logo and a "badge SVG — square mark for favicons and
compact badges"; the site's logo offers "Copy SVG" from its context menu. The project is MIT and states no trademark
terms. The badge is a 4×4 pixel grid whose coordinates are all multiples of 140 in a 560 box; the sizing rule lands it
on 2.5-unit cells exactly, which is why it is the crispest mark in the row.

### OMP — official block-pi

`https://omp.sh/favicon.svg` is a rounded dark tile with a gradient π path; the same bytes are checked in at
`packages/collab-web/public/favicon.svg` in the MIT-licensed `can1357/oh-my-pi`. The site's header component ships a
`monochrome` variant in `currentColor`, evidence that a single-colour rendering is intended. Its newer header geometry
differs from the favicon; Farhelm keeps the pinned favicon's π path and drops the tile. Stencil's
[terms](https://stencil.so/legal/terms), sections 1.3 and 9, put non-hosted open-source software under its repository
license and separately reserve use of Stencil's marks. Their scope is wider than the website. The recorded basis here is
the MIT favicon and the owner's monochrome implementation, with no OMP-specific recoloring prohibition found; that does
not establish a separate trademark license.

An older `assets/icon.svg` in the repository is a different, stale design and is not the mark.

### Grok — Farhelm original

The Grok row uses a plain block G that Farhelm drew itself. The current
[official brand guidelines](https://x.ai/legal/brand-guidelines) offer referential-use terms and a logo kit, requiring
official logos to remain unaltered. This color-only update retains Farhelm's original rather than adopting different
geometry. The G names the harness without tracing or approximating its official mark, and its single filled path stays
legible at the sidebar's 12px size. The inspected official favicon and repository artwork are monochrome; the kit ZIP
was unavailable during the color re-check.

### OpenCode — official favicon cut

`https://opencode.ai/brand` publishes eight logo variants with no usage terms at all, and the repository is MIT. Farhelm
originally vendored the two-path logo file; that design is two-tone, with a mid-grey inner square that collapses under
`currentColor`, and its nested frames leave a one-pixel gap at 12px. The project's favicon (`favicon-v3.svg`) is a
single even-odd ring, the small-size simplification OpenCode itself uses, so the sidebar now uses that path instead.

## Colors

Color identifies an agent, never its permission mode or activity. The following source and terms re-check was made on
2026-11-03. The seven colored marks clear 3:1 against both the sidebar and row hover surfaces in each theme. Light
shades change lightness where contrast requires it; they are Farhelm adjustments, not additional official colors. Every
selected mark uses white (`--on-fill`) instead. Geometry and sizing remain independent of these CSS roles.

| Mark     | Dark / light          | Source and color-use basis                                                                                                                                                                                                                                                                                                                                   |
| -------- | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Claude   | `#d97757` / `#d25e39` | Anthropic's [official palette](https://github.com/anthropics/skills/blob/ca45fc496a3436251f191354091d557da31c15b5/skills/brand-guidelines/SKILL.md) supplies terracotta. It colors Farhelm's original starburst under the maintainer decision above, not the restricted official spark.                                                                      |
| Muse     | `#456bff` / `#456bff` | Farhelm blue. The [community SVG](https://github.com/corona10/muse-code-vscode/blob/6846fc377227ef7667c5eb211019e64338d5762f/resources/muse-logo.svg) is MIT-licensed; its source blue is unofficial. The [product page](https://dev.meta.ai/products/muse-code) and [SDK](https://github.com/meta-models/muse-code-sdk) supplied no Muse Code-specific hue. |
| Cursor   | `#2fbfaf` / `#249084` | Farhelm teal. The [brand page](https://cursor.com/brand) and downloads supply neutral variants. No additional color restriction was found there; `currentColor` is a rendering mechanism, not an express color license.                                                                                                                                      |
| Pi       | `#4d9abf` / `#3e89ad` | Blue from the [primary logo](https://pi.dev/logo.svg) in the [MIT-labeled press kit](https://pi.dev/press-kit). That logo also uses coral `#f09082` and yellow `#f1be58`; Farhelm takes one published hue for its unchanged badge, rather than reproducing the three-color arrangement. No requirement to keep those fills together was found.               |
| OMP      | `#9b4dff` / `#9b4dff` | The middle purple stop of the [official favicon](https://omp.sh/favicon.svg), whose other stops are pink `#ed4abf` and cyan `#5ad8e6`. Purple separates from Pi's blue better than cyan. The MIT source and official monochrome implementation are the basis described above.                                                                                |
| Grok     | `#d8b839` / `#987f1e` | Farhelm gold on its original G. The [official favicon](https://grok.com/images/favicon.svg) is neutral; the current kit's no-alteration rule applies to official artwork, which is not used here.                                                                                                                                                            |
| OpenCode | `#83b94b` / `#638d37` | Farhelm yellow-green. The [brand page](https://opencode.ai/brand) and [favicon](https://opencode.ai/favicon-v3.svg) supply white/grey artwork. The MIT repository and [open-source terms](https://opencode.ai/legal/terms-of-service) remain the copyright basis; no separate color restriction was found on the checked pages.                              |
| Codex    | inherited grey        | The [OpenAI guidelines](https://openai.com/brand/) explicitly forbid adding blossom colors. Keep the existing one-color treatment; inherited grey is not the supplied black or white.                                                                                                                                                                        |
| Goose    | inherited grey        | The [LF Projects policy](https://lfprojects.org/policies/trademark-policy/) still asks that logos have no color variations. Keep the existing one-color treatment described above.                                                                                                                                                                           |
| Terminal | inherited grey        | Farhelm's own non-agent mark; it retains the ordinary agent-track foreground.                                                                                                                                                                                                                                                                                |

For a future harness, check the owner's current terms before assigning color. Keep a mark one-color where those terms do
not allow coloring; otherwise take a published official hue when one exists. A black, white or grey mark supplies no
chromatic hue, so choose a Farhelm hue that stays distinct from the existing set. Clear 3:1 on both sidebar surfaces in
both themes and preserve the selected-row inverse foreground. Record the source, check date and any contrast shade here.
Compare the full set's perceptual separation when tuning it; numeric distances belong in the change's review evidence
rather than this provenance record, where they would go stale.

## Adding or changing a mark

1. Find the owner's official mark and their written terms before drawing anything. Check the brand or press page, the
   repository (favicons and app icons are usually the small-size cut), and any published trademark policy. Record what
   you find here, including a "not found".
2. If the terms permit referential use and the mark survives 12px, vendor the exact path bytes from a pinned upstream
   file, add the attribution and licence text to `THIRD_PARTY_NOTICES.md`, and keep the geometry unaltered.
3. If the terms do not permit it, draw an original that evokes rather than copies, and say so in this file and in the
   constant's doc comment. Do not trace or approximate a trademarked mark: an approximation carries the same exposure
   and looks worse.
4. Compute the transform from the exact bounding box using the rule above, and record the box in the comment beside it.
   Never use an SVG `text` element.

## Host marks

Host identity uses a separate 16-unit, stroke-only box at weight 1.3, rendered at 12px in session and host rows. The
local host starts with the red laptop but can choose a different mark or color. Every host chooses among cloud, house,
flask, database, chip, rocket, gear, gem, hexagon, triangle, ring, square, bug, factory, castle and laptop; these
drawings were made for Farhelm and carry no third-party brand provenance. Keep their silhouette and stroke weight when
adding callers, rather than applying the harness marks' 12-unit normalization rule.

The helm stores stable icon and color words. The seven host color tokens identify a host and carry no connection,
permission or activity state; default uses the ordinary foreground. The SVG is decorative, with the caller's accessible
locality word beside it and an explanation of that locality on hover. Its `data-glyph` is the icon word, so tests
distinguish an actual drawing from a locality label that could agree with the wrong picture.
