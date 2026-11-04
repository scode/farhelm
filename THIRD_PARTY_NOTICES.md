# Third-party notices

## Unicode properties for emoji selectors

The literal variation-base exceptions, formatting ranges and Hangul attachment properties in
`vendor/alacritty_terminal/src/term/mod.rs` derive from Unicode 18.0's `emoji-variation-sequences.txt`,
`auxiliary/GraphemeBreakProperty.txt` and `DerivedCoreProperties.txt`, as bundled by Ghostty's pinned uucode dependency
`ea62149739404a73c202b48a33bf6dd2af4bd9b0`. Copyright © 1991–2026 Unicode, Inc. The
[Unicode License V3](https://www.unicode.org/license.txt) copyright and permission notice is reproduced in
[`vendor/UNICODE-LICENSE.txt`](vendor/UNICODE-LICENSE.txt). No Ghostty implementation is copied.

## Inter interface font

The Inter regular, semibold and bold WOFF2 files in `crates/farhelm-ui/assets/fonts/` are unchanged copies of the
website's vendored Inter fonts. Copyright 2016 The Inter Project Authors
([`rsms/inter`](https://github.com/rsms/inter)). They are licensed under the SIL Open Font License, Version 1.1; the
complete notice is in `crates/farhelm-ui/assets/fonts/OFL-Inter.txt` and ships in the app as `/assets/OFL-Inter.txt`.

## Harness marks

The session sidebar's harness marks in `crates/farhelm-ui/src/icons.rs` vendor geometry from the sources below. Each is
used with unaltered geometry apart from a uniform scale into the shared 12-unit box; `docs/harness-marks.md` records why
each mark was chosen, its color treatment and what the owner's terms allow. Marks not listed here (Claude, Grok,
Terminal) are Farhelm's own drawings.

## OpenCode sidebar mark

The OpenCode mark uses the even-odd ring path from
[`packages/ui/src/assets/favicon/favicon-v3.svg`](https://github.com/anomalyco/opencode/blob/b7b2eae20c7c4276343e5f1f0e5feaeb5ac3a0ef/packages/ui/src/assets/favicon/favicon-v3.svg)
at commit `b7b2eae20c7c4276343e5f1f0e5feaeb5ac3a0ef` of [`anomalyco/opencode`](https://github.com/anomalyco/opencode).
The source is licensed under the MIT License:

> Copyright (c) 2025 opencode

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
> documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
> rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit
> persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
> Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
> WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
> COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
> OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## OMP sidebar mark

The OMP mark uses the π path (without the background tile) from
[`packages/collab-web/public/favicon.svg`](https://github.com/can1357/oh-my-pi/blob/b16bfbe0f9cf953d9a37fc17284967ed18fae015/packages/collab-web/public/favicon.svg)
at commit `b16bfbe0f9cf953d9a37fc17284967ed18fae015` of [`can1357/oh-my-pi`](https://github.com/can1357/oh-my-pi), the
same file served at `https://omp.sh/favicon.svg`. The source is licensed under the MIT License:

> Copyright (c) 2025 Mario Zechner\
> Copyright (c) 2025-2026 Can Bölük\
> Copyright (c) 2026 Stencil Labs, Inc.

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
> documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
> rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit
> persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
> Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
> WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
> COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
> OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Muse sidebar mark

The Muse mark uses the stroked "M" and terminal-dot paths (with a Farhelm hue replacing the source blue, and without the
sparkle) from
[`resources/muse-logo.svg`](https://github.com/corona10/muse-code-vscode/blob/6846fc377227ef7667c5eb211019e64338d5762f/resources/muse-logo.svg)
at commit `6846fc377227ef7667c5eb211019e64338d5762f` of
[`corona10/muse-code-vscode`](https://github.com/corona10/muse-code-vscode), an unofficial community extension that is
not made, endorsed, or supported by Meta. The source is licensed under the MIT License:

> Copyright (c) 2026 Muse Code for VS Code contributors

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
> documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
> rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit
> persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
> Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
> WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
> COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
> OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Goose sidebar mark

The Goose mark uses the path from
[`ui/desktop/src/components/icons/Goose.tsx`](https://github.com/aaif-goose/goose/blob/0c042010ff2303fa81c23cd17de57e9fe62066a1/ui/desktop/src/components/icons/Goose.tsx)
at commit `0c042010ff2303fa81c23cd17de57e9fe62066a1` of [`aaif-goose/goose`](https://github.com/aaif-goose/goose),
copyright the goose contributors, licensed under the Apache License, Version 2.0
(`https://www.apache.org/licenses/LICENSE-2.0`). goose is a Series of LF Projects, LLC; the goose name and logo are used
here only to identify the goose agent, under the LF Projects trademark usage guidelines
(`https://lfprojects.org/policies/trademark-policy/`), and no endorsement is implied.

## Cursor sidebar mark

The Cursor mark is the 2D cube from `CUBE_2D_LIGHT.svg` in the brand asset download at
[`https://cursor.com/brand`](https://cursor.com/brand), with its geometry unaltered apart from scale. The source offers
neutral variants; Farhelm assigns its own identity hue through CSS. Cursor and the cube logo are trademarks of
Anysphere, Inc., used here only to identify the Cursor agent; no endorsement is implied.

## Codex sidebar mark

The Codex mark is the OpenAI blossom from `OpenAI-black-monoblossom.svg` in
[`https://cdn.openai.com/brand/OpenAI-Logos-2025.zip`](https://cdn.openai.com/brand/OpenAI-Logos-2025.zip), the official
download linked from [`https://openai.com/brand/`](https://openai.com/brand/), used unaltered apart from scale under
OpenAI's brand guidelines and marks usage terms on that page. The OpenAI name and logo are trademarks of OpenAI, used
here only to identify the Codex agent; no endorsement is implied.

## Pi sidebar mark

The Pi mark is the badge from the pi.dev press kit ([`https://pi.dev/press-kit`](https://pi.dev/press-kit)), served at
`https://pi.dev/favicon.svg`, with its geometry unaltered apart from scale. Farhelm uses the blue from the press kit's
three-color primary logo, with a darker light-theme shade; it does not reproduce that logo's full color arrangement. The
pi project is MIT-licensed (Copyright (c) 2025 Mario Zechner) and publishes its marks for reuse; no endorsement is
implied.

## Terminal emulator crates

`vendor/alacritty_terminal/` and `vendor/vte/` are the
[`alacritty_terminal`](https://crates.io/crates/alacritty_terminal) 0.26.0 and [`vte`](https://crates.io/crates/vte)
0.15.0 crates from crates.io, part of the [Alacritty](https://github.com/alacritty/alacritty) project (authors Christian
Duerr and Joe Wilm), with Farhelm's changes applied. `vendor/README.md` describes the changes, and `vendor/patches/`
holds them as diffs against the published crates; each modified file says so in a `Modified for Farhelm` comment at its
top. Upstream's reference test data (`alacritty_terminal`'s `tests/`) is not included.

`alacritty_terminal` is licensed under the Apache License, Version 2.0 (`vendor/alacritty_terminal/LICENSE-APACHE`).
`vte` is licensed under the Apache License, Version 2.0 or the MIT License, at your option (`vendor/vte/LICENSE-APACHE`,
`vendor/vte/LICENSE-MIT`, Copyright (c) 2016 Joe Wilm), except `vendor/vte/src/ansi.rs`, which came from
`alacritty_terminal` and is licensed under the Apache License, Version 2.0 only. Farhelm uses both crates under the
Apache License, Version 2.0.
