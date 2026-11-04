# Agent-screen tools use a locale decoder for UTF-8 tmux output

Reviewed commit: 412d403cd21ebb3083747ba530841fcd3d063282

## TLDR

The two agent-screen tools can fail to read a non-ASCII pane title when Python starts with a legacy text encoding and
UTF-8 mode disabled. They request UTF-8 from tmux but leave Python's output decoder dependent on its startup
environment.

## Details

`scripts/agent-tui.py:99–107` and `scripts/capture-agent-screens.py:178–193` include tmux's `-u` flag, then use
`subprocess.run(..., capture_output=True, text=True)` without an encoding. Their environment helpers at
`agent-tui.py:67–75` and `capture-agent-screens.py:688–696` retain locale and Python settings; their `python3` shebangs
do not establish a decoder override. A locale-selected ASCII decoder can reject the UTF-8 bytes these calls now request.
Existing capture-pane output already depended on decoder agreement; this change also makes format/title output UTF-8
instead of sanitized ASCII.

A recorded decoder check, recorder run `418fe160-baf1-4622-a5f1-2128ade013a4`, establishes the mismatch. Python was
started with `LC_ALL=C`, `PYTHONUTF8=0` and `PYTHONCOERCECLOCALE=0`; the helper did not change its own environment. The
inspected output confirms Python 3.15.2, UTF-8 mode 0 and `ANSI_X3.4-1968`. A disposable child emitted bytes `C3 A9`:
default `text=True` raised `UnicodeDecodeError`, while `encoding='utf-8'` returned U+00E9. This establishes the
conditional decoder mismatch, not an end-to-end failure of either actual tool.

Set an explicit UTF-8 encoding on the tmux subprocess reads. A targeted verification should cover the real helper/title
route under the explicit legacy Python startup configuration, while keeping the environment selection outside the test
process. Both scripts remain unchanged through `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`.

## Confidence and priority

Confidence: confirmed conditionally for the inherited decoder mismatch. Actual helper/tmux/agent/title capture was not
run. Modern Python commonly enables or coerces UTF-8 for a bare C locale, so this does not establish failure in an
ordinary Finder launch.

Suggested bucket: `other`, confined to maintainer tooling. `could_be_highest: false`. `proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, owner `g001`, ID `g001-l6`, latest `g001/findings.jsonl` physical
line15. Evidence pointer: recorder run `418fe160-baf1-4622-a5f1-2128ade013a4`, `output-head.log`; helper
`decoder-check.py`.
