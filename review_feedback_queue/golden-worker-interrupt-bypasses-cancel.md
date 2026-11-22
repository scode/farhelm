# Worker-only SIGINT or SIGTERM bypasses screenshot cancellation

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A screenshot worker interrupted on its own can leave the runner packaging a partial result or certifying an update after
the review worker was interrupted.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d scripts/check-gui-goldens.py:196-203,220-244,247-267,285-345;
scripts/golden_changes_page.py:126-167; scripts/test-check-gui-goldens.py:283-317

A renderer stopped by a child-only SIGINT or SIGTERM can still generate a partial review page in either mode. A native
review worker stopped the same way can leave a successful full update marked publishable. run_workers returns the first
nonzero child result without normalizing negative signal statuses, and normalizes a signal handled by the runner itself.
The render path checks for positive stop codes before normalization. Compare mode then packages already-kept captures
and uses its already selected reference directory without a second fetch or native review. It preserves the nonzero
status and the comparison directory. Update can attempt reference lookup and start native review on partial captures,
but its render failure still prevents a completion marker. Only update launches a separate native review worker. Its
child-only negative signal is logged as an ordinary optional-review failure. If the original render succeeded and the
update was full and complete, finish_update can then mark the captures publishable. A selected update remains
unpublishable. A prior ordinary worker failure can mask a later interrupt because the aggregation preserves the first
nonzero result. Wrapper-directed SIGINT or SIGTERM stops correctly during either worker phase. The cancellation
stand-ins exercise positive return codes. Normalize child interruption at collection and give it precedence over
ordinary failures while preserving deliberate best-effort behavior for ordinary review failures. Introduced at
0d52ff68e92cd2bee5b4752d02c3a3210ffbdd5d.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `golden-report/GR-02`.
