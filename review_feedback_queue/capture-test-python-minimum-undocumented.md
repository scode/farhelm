# The standalone capture test gained an undocumented Python minimum

Reviewed commit: 4c18498dd4eab58a550294bf5b3e73852419f9e0

## TLDR

The standalone capture test gained a Python 3.11 dependency while its documented command still only names python3.

## Details

**other.** Source inspection.

4c18498dd4eab58a550294bf5b3e73852419f9e0 scripts/test-capture-agent-screens.py:3–15,91–121;
1a53d9d35889d7dde96e31320cc1681dd013737d .agents/validation.md:58,1160–1163; .agents/run-all-tests.md:136–137

The added capture tests use Python 3.11 APIs. The standalone validation entry invokes `python3` and does not state a
minimum. The nearby explicit version guidance applies to Rust execution and the recorder, which is optional for this
standalone test. Support on Python 3.10 is undocumented. Either name the minimum for the standalone command or use APIs
that fit its intended support. See `scripts/test-capture-agent-screens.py:3–19,91–123` at `4c18498dd`,
`.agents/validation.md:53–62,1160–1164`, and `.agents/run-all-tests.md:76–83,136–137` at the pinned tip.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `capture/capture-test-python-floor`.
