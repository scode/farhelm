# Terminal hyperlink mismatch warnings and concealed cells

## What this was about

Hovering a terminal hyperlink shows where it goes and warns when its label looks like a different web address. The
warning used to mistake some descriptive labels for addresses when a URL immediately followed a parenthesis or bracket.
Concealed text in a program's hyperlink could also produce a false warning or hide a real mismatch. The maintainer
scheduled both fixes together and chose to exclude concealed cells from the warning, underline and click area.

## Things you should know

- A descriptive label can contain a parenthesized URL and still get the ordinary destination display. Wrapped
  descriptions follow the same rule. A label that presents a lookalike address can still warn.
- Concealed characters in a program hyperlink are skipped in the label and underline. They have no hover or click
  target, even when their text looks like a file path.
- A reveal on the hovered row refreshes the warning when it makes a wrapped label ambiguous.
- A label that literally prints `<U+202E>` before a URL gets the ordinary destination display. An address beginning with
  the actual invisible directionality character still warns. The pinned fixtures remain intact.

## Open questions and possible follow-ups

None from this plan.

## PRs

- [#4737](https://github.com/scode/farhelm/pull/4737/changes) Recognize visible web addresses without mistaking
  descriptive labels for addresses.
- [#4738](https://github.com/scode/farhelm/pull/4738/changes) Exclude concealed hyperlink cells and refresh a hover when
  a reveal changes its warning.

## Checks

- The corrected address draft at `de27eea5` passed 55 grid-model tests (`45622e59-6b46-4acc-95eb-dfa7717d271e`).
- At `9d395973`, all 62 grid-model tests (`98f9ffc0-ae99-4e17-a4e5-d5cae06a8543`) and 47 existing desktop terminal link
  and file-disclosure tests (`e132ec22-0331-413b-b9d4-39c8555afd76`) passed. The new style-only reveal test also passed
  in isolation (`bc7000bf-7f22-49f9-b658-2fe891849995`).
- The covered-link and file-disclosure screenshots matched both Linux themes (`eafb75ee-1a52-4db7-9bba-b11dd9136079`).
  Full formatting, strict grid-model Clippy, Markdown and changelog checks passed. The test delay checker found 310
  covered delays and none missing a rationale.
- The pushed draft heads are identical to the tested trees, so their results carry forward. The full workspace suite and
  screenshot catalog were left to landing. The change is in the platform-independent terminal link model and two desktop
  fixture initializers.

<details>
<summary>Recorded runtime commands</summary>

The commands ran through the test recorder. Nextest used four test slots and zero retries.

```sh
cargo nextest run -p farhelm-grid-model
cargo nextest run -p farhelm-grid-model --test links -E 'test(=revealing_a_second_stretch_invalidates_a_wrapped_link_disclosure)'
cargo nextest run -p farhelm-gui --lib -E 'test(/^terminal::(attached::tests::.*(hyperlink|_link|printed_path)|files::tests::)/)'
python3 scripts/check-gui-goldens.py --scene terminal-covered-link --scene terminal-file-disclosures --jobs 2
```

</details>

## Review

Both drafts passed their independent source review after corrections. The first test run
(`401e72c2-c940-4e37-ae51-bafbb5fd1b1d`) caught an ambiguous wrapped fragment that had stopped warning. Review also
caught changes to lookalike schemes, internationalized names and URL credentials. The address change now preserves those
warnings. The concealment review caught a separate stretch whose appearance could change a warning. The added style-only
reveal test pins that refresh.
