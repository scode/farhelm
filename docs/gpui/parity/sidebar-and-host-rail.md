# Parity: Sidebar and host rail

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Session list, Status, Topology.

| Old spec                                   | Tests | Covers                                                                                         | Status         |
| ------------------------------------------ | ----- | ---------------------------------------------------------------------------------------------- | -------------- |
| `tests/sidebar.spec.ts`                    | 116   | the sidebar app bar explains the helm and window builds                                        | not yet ported |
| `tests/sort.spec.ts`                       | 21    | session list ordering                                                                          | not yet ported |
| `tests/filters.spec.ts`                    | 13    | permanent host filtering                                                                       | not yet ported |
| `tests/status-filter.spec.ts`              | 4     | status segments query the helm, count matches and retain a hidden terminal                     | not yet ported |
| `tests/pins.spec.ts`                       | 5     | pin from the menu groups the sidebar and unpin restores title order                            | not yet ported |
| `tests/sidebar-resize.spec.ts`             | 2     | sidebar drag clamps, persists, resets and refits the program                                   | not yet ported |
| `tests/host-rail-warning.spec.ts`          | 7     | ${operation} warning acknowledges once, relatches a new failure and clears on correlated retry | not yet ported |
| `tests/host-short-names.spec.ts`           | 7     | short host captions preserve aliases, abbreviate FQDNs and truncate inside the collapsed rail  | not yet ported |
| `tests/rail-notices.spec.ts`               | 4     | rail notices quiet, alerting and resolved in ${palette}                                        | not yet ported |
| `tests/work-start-order.spec.ts`           | 1     | active sessions lead across bursts, idle demotion, reload and rename                           | not yet ported |
| `tests/bulk-session-actions.spec.ts`       | 14    | marking retains New dispatch and launcher cancellation focus                                   | not yet ported |
| `tests/menu-highlight.spec.ts`             | 5     | session menu shares one highlight between hover and arrow keys                                 | not yet ported |
| `tests/retired-container-sessions.spec.ts` | 1     | retired container rows offer Delete only while new containers keep normal controls             | not yet ported |
| `tests/ui-plan-regressions.spec.ts`        | 5     | header restart and replace refresh the sidebar under build skew                                | not yet ported |
