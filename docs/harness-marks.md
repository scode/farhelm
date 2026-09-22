# Harness marks in the session sidebar

The session list identifies each agent harness with a small mark beside the permission lock. This document records where
every mark comes from, what the brand owner's terms allow, and the one sizing rule they all obey, so that the next
person to touch `crates/farhelm-ui/src/icons.rs` does not have to redo the research or guess why a given harness does
not carry its official logo. Research date: 2026-09-22. Brand terms change; re-check the cited pages before relying on a
statement here for a new use.

## The sizing rule

Every mark is a path inside the shared `viewBox="0 0 12 12"`, wrapped in a `<g transform>` that applies one rule to the
mark's native coordinates: scale so the longer side of its bounding box spans 10 units, then centre it in the box. With
`w`, `h`, `x`, `y` the native bounding box and `s = 10 / max(w, h)`, the transform is
`translate((12 - w*s)/2 - x*s, (12 - h*s)/2 - y*s) scale(s)`. The bounding box is recorded in a comment beside each
transform so the arithmetic can be checked. The mark is then rendered in a 10×12 CSS pixel slot by `.sidebar-glyph`.

The rule exists because the marks are only ever seen next to each other. Before it, three marks were SVG `text` elements
at font-size 9 and drew at roughly three quarters the height of the path marks beside them. Do not render a mark as
text, and do not hand-tune a transform to make a mark "look right": change the bounding box, recompute, and keep the
rule.

Official geometry is scaled by the rule and nothing else. Several of the terms below forbid modification, and an
unaltered path is also what lets `THIRD_PARTY_NOTICES.md` cite an upstream file at a commit rather than describe a
derivative work.

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
| OpenCode | even-odd ring             | official, unaltered    | `packages/ui/src/assets/favicon/favicon-v3.svg` in `anomalyco/opencode` (MIT)   |
| Terminal | window with prompt        | Farhelm original       | drawn here                                                                      |

### Claude — Farhelm original, by constraint

Anthropic's trademark guidelines (`https://www.anthropic.com/legal/trademark-guidelines`, effective 2024-08-01) allow
use of its marks "only as specifically permitted by us and only in materials we approve beforehand", state that "no
alterations of our trademarks (changes to color, font, proportion, or otherwise) are permitted", and offer no
compatibility or referential carve-out. The Claude Code legal page
(`https://code.claude.com/docs/en/legal-and-compliance`) narrows this to: you may say in plain text that a product runs
Claude Code, but "any other use of Anthropic's names or logos ... requires our written permission". Anthropic publishes
no brand kit. The spark SVG is easy to obtain (the VS Code extension ships it, simple-icons carries it), which is not
the same as being allowed to show it recoloured in a third-party sidebar.

So the Claude mark is Farhelm's own drawing: eight tapered rays from a common centre. It is meant to evoke "spark"
without reproducing the spark, whose twelve irregular hand-drawn rays are what make it recognisable. If Anthropic
publishes referential-use terms, revisit; until then, do not swap in the real spark.

### Codex — OpenAI blossom, official

There is no Codex-specific logo. OpenAI brands the CLI and the VS Code extension with the OpenAI blossom, and its brand
page (`https://openai.com/brand/`) is written as a self-serve licence for third parties: use the logo "only when it
directly relates to OpenAI services", "exactly as provided", not "more prominently than your own", and do not "modify it
in any way". Official black and white "monoblossom" SVGs are in `https://cdn.openai.com/brand/OpenAI-Logos-2025.zip`.
The mark is the `d` attribute of `OpenAI-black-monoblossom.svg` byte for byte, kept in `icons/openai-blossom.svgpath`
because a 2.5 KB path does not belong on one source line and must not be simplified. The permission is revocable; if
OpenAI's terms change, the mark comes out.

At 12px the blossom's interlocking arms lose their gaps and it reads mostly by silhouette. That was judged acceptable
against the alternative of inventing something in its place.

### Muse — community icon, because Meta publishes none

Meta publishes no mark for Muse Code. Its product and docs pages (`https://dev.meta.ai/products/muse-code/`) carry only
the Meta corporate lockup and favicon, the official SDK repository has no image assets, and Meta's brand resources say
that "all usage of the Meta logo requires approval" and that Meta's marks "may only be used as provided in these
guidelines or with Meta's permission". The consumer "Muse" app launched in September 2026 with a scribble-M icon, but it
exists only as raster artwork and Meta has not connected it to Muse Code.

The mark is therefore taken from `corona10/muse-code-vscode`, an unofficial VS Code extension under MIT whose README
states it is not made or endorsed by Meta. Its icon is a stroked "M" with round dots at the four terminals and a small
sparkle; Farhelm keeps the M and the dots and drops the colour and the sparkle, which at 12px is a speck. This is a
copyright-clean placeholder, not a brand: it carries no recognition value beyond the letter. Replace it if Meta ever
publishes a Muse Code mark with terms that allow this use.

### Cursor — official cube

`https://cursor.com/brand` offers logo, cube, wordmark, app icon, and avatar downloads. Its only written guidance is
which variants exist and "Refer to us as Cursor. Not Cursor AI or Cursor Code"; there is no trademark policy page, and
the marketplace publisher terms that reference the brand guidelines bind marketplace publishers, not referential use.
The 2D cube is one path, and cursor.com renders the same geometry inline in `currentColor`, so a monochrome rendering
matches the owner's own use. The 2.5D variant is five grey fills and cannot be rendered in one colour; do not use it.

### Goose — official glyph, permitted but coarse

Goose moved from Block to the Agentic AI Foundation under LF Projects in 2026 (`github.com/block/goose` redirects to
`aaif-goose/goose`). The repository is Apache-2.0 with no logo exclusion; trademark use falls under the LF Projects
policy (`https://lfprojects.org/policies/trademark-policy/`), which permits referential use and asks that a logo not be
"displayed with color variations". The mark is single-colour `#101010` upstream, so a `currentColor` rendering is a
recolouring in the letter of that clause; the risk was judged low, but it is not explicitly blessed.

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
`monochrome` variant of the same mark in `currentColor`, the strongest available signal that a single-colour rendering
is intended. Farhelm uses the π path and drops the tile. The only trademark language is Stencil Labs' site terms
("Stencil's names, logos, and marks may not be used without prior written permission"), which govern stencil.so's
services rather than the omp mark; referential use of an MIT-licensed favicon was judged acceptable.

An older `assets/icon.svg` in the repository is a different, stale design and is not the mark.

### OpenCode — official favicon cut

`https://opencode.ai/brand` publishes eight logo variants with no usage terms at all, and the repository is MIT. Farhelm
originally vendored the two-path logo file; that design is two-tone, with a mid-grey inner square that collapses under
`currentColor`, and its nested frames leave a one-pixel gap at 12px. The project's favicon (`favicon-v3.svg`) is a
single even-odd ring, the small-size simplification OpenCode itself uses, so the sidebar now uses that path instead.

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
