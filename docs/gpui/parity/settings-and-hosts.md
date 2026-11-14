# Parity: Settings and hosts

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Topology, Supported host setup, Local authority and trust between hosts.

| Old spec                             | Tests | Covers                                                                           | Status         |
| ------------------------------------ | ----- | -------------------------------------------------------------------------------- | -------------- |
| `tests/settings.spec.ts`             | 27    | settings focus stays on its painted fill and descriptions stay with their labels | not yet ported |
| `tests/credentials-settings.spec.ts` | 3     | credentials keep fixed names, replace write-only values and discard login drafts | not yet ported |
| `tests/credential-delivery.spec.ts`  | 2     | saved credentials report delivery failures without retaining the secret draft    | not yet ported |
| `tests/ntfy-settings.spec.ts`        | 3     | ntfy settings explicitly save, refuse, preserve and clear a token                | not yet ported |
| `tests/tensorlake-settings.spec.ts`  | 6     | Tensorlake settings save a write-only key, test the saved key and clear it       | not yet ported |
| `tests/tensorlake-recipes.spec.ts`   | 10    | Settings keeps recipe answers sticky and isolated to Tensorlake in ${palette}    | not yet ported |
| `tests/tensorlake-hosts.spec.ts`     | 8     | Add host pins inline recipe answers and focuses Name in ${palette}               | not yet ported |
| `tests/tensorlake-lifecycle.spec.ts` | 21    | wake-to-create leaves ${elsewhere} alone (${mode})                               | not yet ported |
| `tests/tensorlake-tunnels.spec.ts`   | 2     | tunnel failures make a connected host amber and show each mapping                | not yet ported |
| `tests/hosts-panel.spec.ts`          | 2     | Hosts swaps ordinary content and preserves filters, sort, selection and details  | not yet ported |
| `tests/host-picker.spec.ts`          | 6     | host picker supports mouse, navigation, typeahead, cancellation, and focus       | not yet ported |
| `tests/host-updates.spec.ts`         | 14    | attention groups problems, work and updates and opens the exact host             | not yet ported |
| `tests/host-checkout-folder.spec.ts` | 1     | host checkout folder saves, inherits, clears and reaches launch preview          | not yet ported |
| `tests/provisioning.spec.ts`         | 73    | rail ${action} modal stays visible over Sessions and returns to its avatar       | not yet ported |
| `tests/ssh-help.spec.ts`             | 4     | ssh help displays and copies both registered-host commands                       | not yet ported |
| `tests/workspace-trust.spec.ts`      | 3     | managed-checkout trust switches persist independently of Claude's legacy route   | not yet ported |
| `tests/appearance.spec.ts`           | 4     | ${initial.name} before application mount                                         | not yet ported |
