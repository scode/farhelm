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

### Landing

#### What landed with it

The plan's two PRs (#4737, #4738) and one landing fix, [#4739](https://github.com/scode/farhelm/pull/4739/changes),
landed alone in their round. Nothing landed on main between the plan's base and its landing except plan bookkeeping, so
nothing could interact with it.

#### Fixes made while landing

- **Hidden characters inside an address-looking label stopped the mismatch warning.** To stop false warnings on labels
  like `ticket-42(https://example.test/tickets/42)`, #4737 made the scheme, credentials and host of an address-looking
  label come from allow-listed characters. That also made a label stop counting as an address when it carried a
  zero-width character anywhere in its scheme, host or credentials, a combining mark (such as a variation selector) in
  its scheme or credentials, or a fullwidth `@`. The terminal keeps zero-width characters with the cell before them, so
  such a label looks exactly like `https://example.test/x`, or like `example.test@other.test/x` with a familiar name as
  credentials, while the link goes elsewhere. Before the plan these all warned. The report's statement that the address
  change preserves the lookalike and credentials warnings was therefore not true as delivered. #4739 has the address
  test skip characters that draw nothing of their own and read a fullwidth or small `@` as `@`, while the comparison
  still uses the label as written, so all of these warn again. It adds a test for each case.
- #4739 also corrects the changelog fragment: concealed text stops responding to hover and click only inside a program's
  hyperlink, since concealed plain text that looks like a file path still opens. It adds the bracket caveat below to the
  fragment. In SPEC.md's definition of an address-looking label, "it has a scheme" becomes "it starts with a scheme",
  since on a plain reading `ticket-42(https://...)` "has a scheme". It also clarifies two code comments.

#### Checks

The plan changes the desktop terminal's link model and two desktop fixtures, so the landing ran the tests of the link
model, the desktop GUI and the input mapping crate that uses the model, both default lints, every reference picture
(link underlines and hover displays are drawn) and the source checks. Worker runs were on a separate arm64 Linux test
worker; the rest ran locally on x86_64 Linux. No test run failed. One source check did: `dprint check` asked for the
SPEC.md paragraph the fix edited to be rewrapped, which was done before the fix was committed.

On the plan's two PRs rebased onto main, before the landing fix:

- `cargo nextest run -p farhelm-grid-model -p farhelm-gui -p farhelm-input-mapping`, on the test worker (recorder run
  `8f78c907-a53d-4105-b3a3-b692e6be20ca`): all 3445 passed. Nextest skipped four ignored tests: three that belong to the
  reference pictures, which run separately below, and a helper that writes mark images.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `2507e848-01e2-45ba-a2a5-53ab66d65684`): every reference
  picture matched.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4739):

- `cargo nextest run -p farhelm-grid-model`, locally (recorder run `589f111d-10ae-42ec-a8af-8357770f024d`): all 63
  passed, including the new hidden-character test and the old web client's table of labels and expected warnings.
- `cargo nextest run -p farhelm-gui --lib` selecting the terminal hyperlink, printed path and file disclosure tests,
  locally (recorder run `1962aa48-0338-44a2-b741-9f303e9bf934`): all 47 passed.
- `cargo clippy -p farhelm-grid-model -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the
  sleep check, `dprint check` and the changelog format check, locally: passed.

Reused for the fix: the rest of the GUI and input mapping tests, the workspace lints and the reference pictures above.
The fix changes only the rule that decides whether a label looks like an address, which only the hover display's warning
uses; the link model's own tests exercise that rule, and the GUI tests that use it were rerun. It changes nothing drawn,
since none of the reference pictures shows a label with hidden characters, so the pictures still apply. Skipped: the
rest of the workspace suite, since nothing outside the desktop GUI uses the link model; the end-to-end desktop smoke,
whose steps do not hover program hyperlinks; macOS, since the change is in the link model, which is the same code on
every platform, and no Mac was used; and CI, which runs only on demand.

#### Things you should know that the report does not say

- **A label that opens with a bracket or quote no longer warns.** `<https://good.test/>`, `"https://good.test/"`,
  `(https://good.test/)` and `[https://good.test/]` on a link that goes to a different site warned before this plan. Now
  they get the ordinary display, which still names the real destination's host. This follows from the plan's scheme
  rule, under which `(`, `[` and `<` cannot start a scheme. The report mentions only one such case, a label from the old
  web client's test table, as if it were a one-off. The changelog fragment now names the whole class. Options:
  - Accept it. The real host is always shown.
  - Have the address test skip one leading bracket or quote. That brings the warning back for these labels, but an
    honest bracketed label would then warn too, unless the matching closing bracket is also dropped before the
    comparison.

  I recommend the second, as a small follow-up that also drops the closing bracket. `<https://...>` is a common way to
  write a link, and a quoted or bracketed familiar address pointing elsewhere is exactly what the warning is for.
- Rarer address-looking labels also stopped warning, for the same reason. These include a scheme-less host containing
  `~`, `ht_tps://example.test/x`, and a wrapped piece that starts with `@` or `:`. All look odd on screen, so I suggest
  no follow-up for them on their own; the bracket follow-up above would not change them.
- Two older gaps the review noticed, which predate this plan. Neither needs to wait on anything here; a follow-up is
  optional:
  - Concealed plain text that looks like a file path is still a clickable link. A concealed printed web address is not.
    Giving file paths the web address rule would close it.
  - A printed file path is blocked by a program hyperlink only at the pointer's own cell. A path that partly overlaps a
    hyperlink can be hovered from its other cells and is then underlined across the hyperlink's cells. Checking the
    whole path's cells would close it.
