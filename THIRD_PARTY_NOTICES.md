# Third-party notices

## Unicode properties for emoji selectors

The literal variation-base exceptions, formatting ranges and Hangul attachment properties in
`vendor/alacritty_terminal/src/term/mod.rs` derive from Unicode 18.0's `emoji-variation-sequences.txt`,
`auxiliary/GraphemeBreakProperty.txt` and `DerivedCoreProperties.txt`, as bundled by Ghostty's pinned uucode dependency
`ea62149739404a73c202b48a33bf6dd2af4bd9b0`. Copyright © 1991–2026 Unicode, Inc. The
[Unicode License V3](https://www.unicode.org/license.txt) copyright and permission notice is reproduced in
[`vendor/UNICODE-LICENSE.txt`](vendor/UNICODE-LICENSE.txt). No Ghostty implementation is copied.

## Inter interface font

The Inter regular and semibold TrueType files in `crates/gui/farhelm-gui/assets/fonts/`, which the GPUI desktop app
bundles, are decompressed byte for byte (`woff2_decompress`) from the website's vendored Inter WOFF2 files in
`website/public/fonts/`. Copyright 2016 The Inter Project Authors ([`rsms/inter`](https://github.com/rsms/inter)). They
are licensed under the SIL Open Font License, Version 1.1; the complete notice is `OFL-Inter.txt` beside them.

## Code ported from xterm.js

The supervisor's grid input encoder is a Farhelm port of code from `@xterm/xterm` 6.0.0. The port keeps behavior that
programs already rely on, such as the bytes each key sends, so it was carried over rather than rewritten. It lives in
`crates/farhelm-supervisor/src/grid_input.rs`: key encoding (`evaluateKeyboardEvent`), mouse report encoding
(`CoreMouseService`, and the wheel on the alternate screen), and paste line endings (`prepareTextForTerminal`). The old
Dioxus UI's JavaScript grid client carried further ports from xterm.js, its `@xterm/addon-fit` 0.11.0 and
`@xterm/addon-web-links` 0.12.0 add-ons and Visual Studio Code's `vs/base`; those were deleted with that UI.

The GPUI app's own Rust ports of those JavaScript rules live in `crates/gui/farhelm-grid-model` (`src/selection.rs` and
`src/links.rs`: selection text, word selection, the URL pattern and line window) and `crates/gui/farhelm-input-mapping`
(`src/keys.rs`, `src/pointer.rs` and `src/cursor.rs`: the keydown decision, mouse cell rounding, move de-duplication,
wheel accumulation and the Alt-click cursor move). Their complete notices are in each crate's `LICENSE-PORTED`.

Each ported function names its origin in a source comment. The code is MIT-licensed: copyright the xterm.js authors,
SourceLair Private Company and Christopher Jeffrey. The complete notices are in
`crates/farhelm-supervisor/grid-ported-code-LICENSES.txt`. They used to ship inside the web bundle that release builds
of `farhelm` embedded; that bundle is gone, so no binary carries them at the moment.

## Harness marks

The session sidebar's harness marks in `crates/gui/farhelm-gui/src/marks.rs` vendor geometry from the sources below.
Each is used with unaltered geometry apart from a uniform scale into the shared 12-unit box; `docs/harness-marks.md`
records why each mark was chosen, its color treatment and what the owner's terms allow. Marks not listed here (Claude,
Grok, Terminal) are Farhelm's own drawings.

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

## GPUI crates

`vendor/gpui-pre/`, `vendor/gpui-pre-linux/`, `vendor/gpui-pre-wgpu/` and `vendor/gpui-pre-macos/` are the
[`gpui-pre`](https://crates.io/crates/gpui-pre) 0.3.12 crates from crates.io (`gpui-pre-linux`, `gpui-pre-wgpu` and
`gpui-pre-macos` at the same version): a published snapshot of GPUI, the UI framework of the
[Zed](https://github.com/zed-industries/zed) editor, at Zed revision `d2a4fb6ded7c470be25d1541281da3882495b65b`, with
Farhelm's changes applied. `vendor/README.md` ("GPUI") describes the changes, and `vendor/patches/gpui/` holds them as
patches against the published crates; each changed or added file says so in a `Modified for Farhelm` comment at its top.
The crates are licensed under the Apache License, Version 2.0 (each tree's `LICENSE-APACHE`).

## gpui-agent

`vendor/gpui-agent/` is [`wsp-labs/gpui-agent`](https://github.com/wsp-labs/gpui-agent) at commit
`c9b8b6328877755685e1233b3770f78a68f2a5d1`, the test-automation bridge for GPUI apps, with Farhelm's changes to its
manifests, compile guards and lockfile (`vendor/patches/gpui/gpui-agent/`) and without its `docs/`, `scripts/` and
`patches/` directories; each changed file says so in a `Modified for Farhelm` comment at its top. It is licensed under
the Apache License, Version 2.0 (`vendor/gpui-agent/LICENSE`). Only builds with the GUI's automation feature (still to
come) will contain it; the GPUI patch layers that it needs are compiled into every build (`vendor/README.md`, "GPUI").

## GPUI Kit icons

GPUI Kit's `gpui-kit-assets` 0.9.1 crate, a dependency of `gpui-component`, bundles the [Lucide](https://lucide.dev)
icon set. Its manifest declares Apache-2.0, but the icons themselves are licensed under the ISC License (Copyright (c)
2026 Lucide Icons and Contributors), and the icons Lucide derived from the [Feather](https://feathericons.com) project
are licensed under the MIT License (Copyright (c) 2013-present Cole Bemis). The complete notices are in that crate's
`LICENSE-LUCIDE`. cargo-deny checks only the declared license, so this entry records the icons' own terms; a build that
ships the icons must carry that file's notices.
