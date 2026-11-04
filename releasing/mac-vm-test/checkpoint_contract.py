"""Compile the selected test path into command obligations shared by all layers.

The host selects an update path from release facts. The UI model must not have
to decide which conditional recipe paragraphs apply, and the command gate and
result validator must agree on that decision.

The terminal transition (old sessions in tmux, new ones in terminal hosts) is
the one place where a P0 checkpoint branches on something observed during the
run rather than chosen by the host: the old session's terminal kind. That
branch, and whether a Restart was due and moved the session, are decided by
samples the fixed operations take themselves at moments they can check
(`host_survival` before the old session is touched, `old_session_after` after
it), compared with the old fixture's digest-bound receipts. The UI model's
reading of session details is a further check, never the decision, and the
order in which it calls observations can turn a pass into a refusal but never
a refusal into a pass.
"""

import json


# Each side of the pair has its own fixed observation of terminal hosts and of
# session rows; the old-session checkpoint for that side compares against the
# old fixture's receipts of the same operations.
SIDES = {
    'local': {'hosts': 'terminal_hosts', 'survival': 'host_survival', 'after': 'old_session_after',
              'state': 'inspect_state', 'agent_check': 'local-supervisor-agent', 'old_check': 'local-old-session'},
    'remote': {'hosts': 'remote_terminal_hosts', 'survival': 'remote_host_survival',
               'after': 'remote_old_session_after', 'state': 'remote_inspect_state',
               'agent_check': 'remote-supervisor-agent', 'old_check': 'remote-old-session'},
}

# Checkpoints known to need longer than the ordinary default turn, as a floor
# under the operator's `--timeout` (capped at the tester's 3600 s maximum).
# The updater pair's old fixture installs, launches and provisions as before and
# now also waits two genuine Codex conversations through capture; each
# old-session check runs two agent turns around an agent Restart.
TURN_FLOORS = {('old-fixture', 'p0-updater-upgrade'): 2400,
               ('local-old-session', 'p0-updater-upgrade'): 1800,
               ('remote-old-session', 'p0-updater-upgrade'): 1800}


def turn_timeout(check, pass_id, requested):
    """Give a checkpoint at least its known floor, never more than the tester's maximum."""
    return min(3600, max(requested, TURN_FLOORS.get((check, pass_id), 0)))


def old_session_side(check):
    """Name the pair side an old-session checkpoint observes, or None for any other checkpoint."""
    return next((side for side, names in SIDES.items() if names['old_check'] == check), None)


def host_identity(host):
    """Key one observed terminal host by pid and platform start identity, never by pid alone.

The start identity is a string on the Mac and a dict on Linux; canonical JSON
compares both the same way across receipts.
"""
    return host['pid'], json.dumps(host['start'], sort_keys=True)


def require_agent_terminal(observation):
    """Refuse a new-session check whose stored terminal kind contradicts the hosts it sampled.

The operation sampled both together, after the new session existed. Where the
schema stores `terminal_kind`, a new `host` row needs a new host process and a
new `tmux` row none. Without the column this is the UI model's to compare.
"""
    kinds = [row['terminal_kind'] for row in observation.get('new_sessions', []) if 'terminal_kind' in row]
    if kinds and ('host' in kinds) != bool(observation['new_hosts']):
        raise ValueError(f'new sessions store terminal_kind {kinds}, which the terminal hosts contradict')


def require_old_session(observations, side):
    """Refuse an old-session pass whose machine-owned samples contradict it.

The survival comparison must hold in both branches: the old session keeps its
id and launch generation across the upgrade, and in the host branch its host
and child are the same processes. Stored terminal kinds must agree with the
observed branch and with what the candidate creates. With no Restart due (tmux,
and the candidate creates no hosts), the final sample must still show the old
launch generation. With a Restart due, it must show a later generation with a
captured conversation, exactly one host that appeared since the survival
sample, a stored kind of `host` where the schema has one, and in the host
branch the old fixture's host gone. A Restart back into tmux is refused.
Whether the resumed agent remembers the earlier turn stays the UI's to report.
"""
    names = SIDES[side]
    survival = observations[names['survival']]
    after = observations[names['after']]
    if survival['kind_disagreements']:
        raise ValueError('; '.join(survival['kind_disagreements']))
    if survival['survived'] is not True:
        raise ValueError('the old session or its terminal host did not survive the upgrade')
    old, row = survival['old_session'], after['session']
    if row is None:
        raise ValueError('the old session is gone after the check')
    if type(row.get('generation')) is not int or type(old.get('generation')) is not int:
        raise ValueError('the old session has no comparable launch generation')
    if not survival['restart_required']:
        if row['generation'] != old['generation']:
            raise ValueError('the old session relaunched although no Restart was due')
        return
    if row['generation'] <= old['generation'] or not row.get('captured_conversation'):
        raise ValueError('the old session restart and resume premise was not established')
    if len(after['appeared_hosts']) != 1 or row.get('terminal_kind', 'host') != 'host':
        raise ValueError('Restart did not move the old session to one new terminal host')
    if after['old_host_present']:
        raise ValueError('the old terminal host is still running after Restart')


def fixed_operations(task):
    """Identify command-only checkpoints whose acceptance needs no UI judgment.

Keep this whitelist smaller than the command API. In particular, installation
inside a GUI upgrade checkpoint still belongs to the native tester's sequence.
The enclosing executor owns deadlines, cancellation and retained failures for
these child processes just as it does for model-driven checkpoints.
"""
    if task['gui'] or not task['context'].get('managed_operations'):
        return None
    return {'artifact-verification': ('verify_candidate',),
            'fixture-clean': ('fixture_clean',), 'install': ('install_candidate',)}.get(task['check_id'])


def required_operations(check, pass_id, update_path):
    """Return independent command premises for exactly this checkpoint and path."""
    requirements = {
        'native-preflight': {'prerequisites'}, 'artifact-verification': {'verify_candidate'},
        'fixture-clean': {'fixture_clean'}, 'install': {'install_candidate'},
        'app-start': {'launch', 'versions'},
        'old-fixture': {'fixture_clean', 'verify_previous', 'install_previous', 'launch', 'versions'},
        'update-discovery': {'update_logs', 'versions'}, 'update-relaunch': {'versions'},
        # Evidence of the new session's terminal, checked against its stored kind.
        'local-supervisor-agent': {'versions', 'terminal_hosts'},
        'remote-supervisor-agent': {'remote_version', 'remote_terminal_hosts'},
        # The survival comparison samples before the session is touched and runs
        # once; the final sample is refused unless the comparison ran first.
        'local-old-session': {'prior_receipts', 'host_survival', 'old_session_after'},
        'remote-old-session': {'prior_receipts', 'remote_host_survival', 'remote_old_session_after'},
        'remote-host-remove': {'remote_host_before', 'remote_host_after', 'remote_version'},
        'remote-host-uninstall': {'remote_host_before', 'remote_host_after'},
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
    if check == 'old-fixture' and pass_id == 'p0-updater-upgrade':
        # P0's one deliberate old session per host: its session row, captured
        # conversation and terminal identities are the later checks' baseline.
        result |= {'inspect_state', 'remote_inspect_state', 'terminal_hosts', 'remote_terminal_hosts'}
    if check == 'update-relaunch' and update_path == 'manual':
        result.add('install_candidate')
    if check == 'update-relaunch' and update_path == 'override':
        result.add('public_latest')
    if update_path == 'override' and (check == 'update-discovery' or (
            check == 'update-relaunch' and pass_id.startswith('upgrade-'))):
        result.add('launch_override')
    return result


def require_remote_host_effect(check, before, after):
    """Distinguish forgetting a host from uninstalling its managed resources.

Both menu checks must start with a real provisioned host and retained data.
Remove must leave the original service and files alone; uninstall must remove
them while keeping the data. Native images separately prove the registry and
session behavior, which these independent SSH observations cannot establish.
"""
    service = before['service']
    if (service['ActiveState'] != 'active' or service['SubState'] != 'running'
            or not service['MainPID'].isdigit() or int(service['MainPID']) <= 0
            or not all(before[key] for key in ('lib_present', 'unit_present', 'binary_sha256',
                                              'unit_sha256', 'data_present', 'database_present'))):
        raise RuntimeError('remote menu check did not start from a provisioned running host with data')
    if not after['data_present'] or not after['database_present']:
        raise RuntimeError('remote menu action lost the retained data directory or database')
    if check == 'remote-host-remove':
        if before != after:
            raise RuntimeError('Remove changed remote resources or the running supervisor')
    elif check == 'remote-host-uninstall':
        if (after['lib_present'] or after['unit_present'] or after['binary_sha256'] or after['unit_sha256']
                or after['service']['MainPID'] != '0' or after['service']['ActiveState'] != 'inactive'
                or after['service']['LoadState'] != 'not-found'):
            raise RuntimeError('Uninstall left remote binaries, unit or supervisor running')
    else:
        raise ValueError('not a remote host menu checkpoint')


def available_operations(check, pass_id, update_path):
    """Offer only this task's mutations plus bounded independent observations.

Read-only observations can help diagnose an unexpected UI state. Installers,
launches and continuity milestones belong only to their declared checkpoint.
"""
    result = required_operations(check, pass_id, update_path) | {
        'prior_receipts', 'versions', 'remote_version', 'update_logs', 'inspect_state',
        'remote_inspect_state', 'terminal_hosts', 'remote_terminal_hosts'}
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
Discovery must prove completed installation, not win a race to capture a
progress indicator that a fast installer may replace before the next image.
Override removal depends on the version returned by the new app's check, not
whether the native provider can reveal a particular tooltip in an image.
"""
    if update_path not in ('manual', 'ordinary', 'override'):
        raise ValueError('an update checkpoint needs a settled update path')
    discovery = '''Click the old app's actual Check for updates. Wait for observed candidate
installation and Restart to update. Call update_logs and versions to retain
installer and installed-version evidence. Capture the completed native Restart
to update offer. An installing-state screenshot is optional: do not require a
transient progress image or fail when installation finishes before capture.'''
    if update_path == 'override':
        discovery = '''Quit the old app normally through native CUA, then call launch_override.
Call update_logs and require the startup override announcement.\n''' + discovery
    restart = '''Wait for the actual Restart to update offer. Use brick_ui method restart with
coordinates from a fresh native image of that menu: args [[x,y]]. The harness
owns the single native click and process handoff.
Require the candidate returns usable, dismiss welcome with got it if present
and observe local connection. An old app that runs its supervisor detached
leaves it running when it quits, and the candidate replaces it on first launch,
so a brief connecting state is the handover: wait for the connected candidate
local supervisor, never stop one yourself. Never manually launch or use getApp to rescue a
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
to Check for updates through native CUA. Require fresh native UI text naming
the exact public default as the latest release, rather than the candidate.
The update control's accessibility text or its visible tooltip can establish
that version; retain the native observation and a candidate acceptance image.
A tooltip image is optional when the accessibility text names the version.
An Up to date message alone does not distinguish the public default from the
candidate. If the endpoint moved to the candidate, report the indistinguishable
override-removal observation as a gap.'''
    return result


def remote_host_instructions(check):
    """Exercise the remote menu before feedback, using the agent fixture already proved by P0.

Remove runs first while both genuine remote sessions are still alive. Uninstall
then ends those sessions through the product before confirming the destructive
plan. The fixed SSH operations observe effects only; they never remove files,
stop services or substitute for either menu item.
"""
    if check == 'remote-host-remove':
        return '''Call prior_receipts for the connected candidate Linux host and its genuine
remote Codex sessions, including the old-session check's migrated/resumed one.
Record that host/supervisor and both session identities from the native UI.
Leave both remote sessions alive; do not Stop, delete or uninstall either.
Call remote_host_before BEFORE selecting Remove.
Open this exact remote host's menu and choose Remove, handling its ordinary
confirmation if offered without changing confirmation preferences. Require the
remote host and its cached sessions disappear from Farhelm; retain a native image.
BEFORE re-adding anything call remote_host_after: its independent SSH snapshot
must show the same running supervisor PID, binary, unit and retained database.
Then add context.ssh.alias through Farhelm's ordinary Add host GUI. Require
discovery of the existing candidate supervisor and the SAME remote sessions,
including their previous conversations, without installing, updating or
creating replacement sessions. Call remote_version and retain native images
showing the reconnected candidate host and recovered sessions. Local host and
local session remain available. This proves Remove forgets the registration
without uninstalling or ending the remote session.'''
    if check == 'remote-host-uninstall':
        return '''Call prior_receipts for the re-added candidate Linux host/session. Through
Farhelm's native UI Stop BOTH remote sessions, the old migrated/resumed one
and the new agent check's one, confirming if offered. Require all remote
sessions ended and close any remote terminal tabs; keep their data. Do not
delete their data or stop a service through commands. Local host and
local session stay untouched. Call remote_host_before while the remote
supervisor is still connected/running and its data/database still exist.
Open this exact remote host's menu and choose Uninstall. Inspect the actual
plan BEFORE confirming: it removes the supervisor service/unit and private lib
directory and explicitly KEEPS the Farhelm data directory. Retain a native
image of that plan. Confirm once through its ordinary Uninstall action, without
changing confirmation preferences. Wait for completion and require the remote
host/cached sessions disappear and the kept-data removal notice appears.
Call remote_host_after; require no remote lib/binary or unit file, no loaded or
running supervisor, and the original data directory/database still present.
Retain a final native image of completion and the usable candidate app/local
host. Do not re-add, reinstall, retry an uncertain uninstall or remove files to
manufacture success. P1 feedback follows on this still-running Mac candidate.'''
    raise ValueError('not a remote host menu checkpoint')


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
Click the new-terminal button (+) after the tabs once. Click agent tab and verify selection, click shell tab and
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
