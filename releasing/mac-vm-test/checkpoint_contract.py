"""Compile the selected test path into command obligations shared by all layers.

The host selects an update path from release facts. The UI model must not have
to decide which conditional recipe paragraphs apply, and the command gate and
result validator must agree on that decision.
"""


def required_operations(check, pass_id, update_path):
    """Return independent command premises for exactly this checkpoint and path."""
    requirements = {
        'native-preflight': {'prerequisites'}, 'artifact-verification': {'verify_candidate'},
        'fixture-clean': {'fixture_clean'}, 'install': {'install_candidate'},
        'app-start': {'launch', 'versions'},
        'old-fixture': {'fixture_clean', 'verify_previous', 'install_previous', 'launch', 'versions'},
        'update-discovery': {'update_logs', 'versions'}, 'update-relaunch': {'versions'},
        'local-supervisor-agent': {'versions'}, 'remote-supervisor-agent': {'remote_version'},
        'remote-provision': {'remote_version'}, 'codex-conversation': {'inspect_state'},
        'seed-state': {'inspect_state', 'versions'},
        'continuity-before': {'continuity_before'}, 'preserved-state': {'inspect_state'},
        'window-drag': {'prior_receipts', 'window_frame'},
        'codex-resume': {'inspect_state'},
        'remote-update': {'remote_version', 'continuity_remote_before', 'continuity_remote_after'},
        'reopen': {'launch', 'versions', 'remote_version', 'continuity_reopen_before', 'continuity_reopen_after'}}
    result = set(requirements.get(check, {'prior_receipts'}))
    if check == 'native-preflight' and not pass_id.startswith('p0-'):
        result.add('finder_frame')
    if check == 'old-fixture' and pass_id != 'p0-installer-upgrade':
        result.add('remote_version')
    if check == 'update-relaunch' and update_path == 'manual':
        result.add('install_candidate')
    if check == 'update-relaunch' and update_path == 'override':
        result.add('public_latest')
    if update_path == 'override' and (check == 'update-discovery' or (
            check == 'update-relaunch' and pass_id.startswith('upgrade-'))):
        result.add('launch_override')
    return result


def available_operations(check, pass_id, update_path):
    """Offer only this task's mutations plus bounded independent observations.

Read-only observations can help diagnose an unexpected UI state. Installers,
launches and continuity milestones belong only to their declared checkpoint.
"""
    result = required_operations(check, pass_id, update_path) | {
        'prior_receipts', 'versions', 'remote_version', 'update_logs', 'inspect_state'}
    if check == 'continuity-before':
        result.add('continuity_command')
    if check == 'window-drag':
        result.add('finder_frame')
    return result


def require_applicable_operation(check, pass_id, update_path, operation):
    """Keep an irrelevant override probe from manufacturing an unavailable manual upgrade.

The latest endpoint was observed by the host at path selection. Only a real
override run needs another endpoint observation to distinguish override removal.
"""
    if operation == 'public_latest' and (check != 'update-relaunch' or update_path != 'override'):
        raise RuntimeError('public_latest is only an override-removal operation at this checkpoint')
    if operation not in available_operations(check, pass_id, update_path):
        raise RuntimeError('operation cannot substitute for this selected checkpoint')


def update_instructions(check, pass_id, update_path):
    """Give the UI model one settled update path, with no inapplicable branches.

P0 discovery and relaunch are separate checkpoints. Bonus upgrades combine
them, while the installer fixture never carries override-removal obligations.
"""
    if update_path not in ('manual', 'ordinary', 'override'):
        raise ValueError('an update checkpoint needs a settled update path')
    discovery = '''Click the old app's actual Check for updates. Wait for observed candidate
installation and Restart to update. Call update_logs and versions to retain
installer and installed-version evidence. Capture native installing/ready states.'''
    if update_path == 'override':
        discovery = '''Quit the old app normally through native CUA, then call launch_override.
Call update_logs and require the startup override announcement.\n''' + discovery
    restart = '''Wait for the actual Restart to update offer. Use brick_ui method restart with
coordinates from a fresh native image of that menu: args [[x,y]]. The harness
owns the single native click and process handoff.
Require the candidate returns usable, dismiss welcome with got it if present
and observe local connection. Never manually launch or use getApp to rescue a
missing restart. Call versions after the candidate returns. Capture the final
native acceptance state. Remote upgrade belongs to its later checkpoint.'''
    if check == 'update-discovery':
        if update_path == 'manual':
            raise ValueError('manual installation cannot satisfy updater discovery')
        return 'Call prior_receipts for established premises.\n' + discovery + '\nDo not restart in this checkpoint.'
    if update_path == 'manual':
        return 'Call prior_receipts. Call install_candidate while the old app runs.\n' + restart
    combined = pass_id.startswith('upgrade-')
    result = 'Call prior_receipts for established premises.\n' + (discovery + '\n' if combined else '') + restart
    if update_path == 'override':
        result += '''\nThe retained discovery receipt establishes the startup override announcement;
do not infer an old temporary log path. Call public_latest, ask the candidate
to Check for updates through native CUA, and capture its actual hover naming
the public default rather than the candidate. If the endpoint moved to the
candidate, report the indistinguishable override-removal observation as a gap.'''
    return result


def window_instructions():
    """Pair every title-bar acceptance state with native pixels and independent window-server facts."""
    return '''Call prior_receipts. Bind the existing candidate Farhelm window through brick_ui.
Use an ordinary window; if fullscreen, report could-not-run. Select an existing
Codex/agent session so the terminal-tab and action row exists; create none.
Call window_frame to inspect the actual owned window and single-display bounds.
If it fills the desktop, use Window > Zoom once through native UI to restore an
ordinary frame; if no ordinary frame with room to move can be established, stop.

Take EXACTLY ten native screenshots S1..S10 and no others on a passing path.
After EACH screenshot call window_frame BEFORE another input action. That
receipt pairs the actual image with native window ID/PID, global position and
size, and image_scale. Convert screenshot-pixel drag offsets to window-server
units with that scale; never infer motion from an unchanged cropped image.
Display pixel counts are not the native capture scale. Frame-only queries do
not take screenshots. Query after EVERY control input/close as well, requiring
unchanged bounds throughout; no extra screenshots are needed.

S1: Ordinary unzoomed frame with room to move. Capture and measure.
S2: Through the native drag method, press empty sidebar top-bar background
(between wordmark and version is an example), move about 150 image pixels right
and 100 down while held, release. Choose another direction if needed to stay on
screen. Capture and measure; require that offset and unchanged window size.
S3: Repeat on empty background of the main pane's top row, away from buttons
and tabs, moving about 100 left and 50 up. Capture and measure the same contract.
S4: Double-click empty sidebar top-bar background with the documented native
clickCount:2 option. Capture and measure a clearly larger zoomed frame.
S5: In that zoomed window find empty band background again, double-click it,
then capture and measure restoration of the frame from before S4.
S6 and S7: Repeat zoom and restoration on empty main-pane top-row background,
capturing and measuring each state.

Press and move about 120 image pixels right while held on the agent tab, then
on delete, releasing each. Query window_frame after EACH; require unchanged
frame. A control must never drag the window. Inspect and dismiss any ordinary
panel before its following click test, without deleting a session.
Open + terminal once. Click agent tab and verify selection, click shell tab and
verify selection (S8, capture/measure), then click agent tab again. Click delete
and require its confirmation to open with unchanged frame (S9, capture/measure).
Cancel without deleting, then close the shell tab with its own close control,
confirming that close if asked.
Press and move on the sidebar settings gear the same way; query window_frame
and require unchanged frame. Close any ordinary panel it opened before the click
test. Click the gear once and require Settings opens with unchanged frame
(S10, capture/measure), then close Settings normally.
Measure once more after that close and require the frame unchanged.

If a background drag leaves the frame unchanged, before classifying product
fail bind Finder, capture its window image and call finder_frame before/after
the same title-bar drag. Finder must have an ordinary file window, not just
Desktop: inspect fresh AX state and use its native File > New Finder Window
or super+n if needed, then verify that window before measuring or dragging.
If Finder also does not move, report could-not-run:
the input did not reach the guest. If a double-click does not zoom or restore,
test the same double-click on an unselected folder in Finder icon view; require
actual navigation. A Finder title-bar double-click is not an oracle because a
user setting can disable it. Unsupported/refused gestures mean could-not-run.
A control moving the window, or not opening on its explicit click, is fail.
Retain the measured frame history and actual native images; no file browsing.'''
