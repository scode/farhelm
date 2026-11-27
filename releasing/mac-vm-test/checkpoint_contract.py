"""Compile the selected test path into command obligations shared by all layers.

The host selects an update path from release facts. The UI model must not have
to decide which conditional recipe paragraphs apply, and the command gate and
result validator must agree on that decision.

P1's old-session checkpoints accept only on process samples taken during the
run: whether the old session and its terminal host survived the upgrade, and
whether its Restart moved it onto one new terminal host. Samples the fixed
operations take themselves at moments they can check (`host_survival` before the old session is touched, `old_session_after`
after it), compared with the digest-bound receipts of `old-sessions`, which
started those sessions on the previous release, decide that.
The UI model's observations are further checks, never the decision, and the
order in which it calls observations can turn a pass into a refusal but never
a refusal into a pass.
"""

import json


REMOTE_MAC_PASS = 'p1-remote-mac'


def require_new_desktop_pids(before, launched, observed):
    """Bind an observation to a newly launched desktop, distinct from the previous app."""
    if any(not isinstance(pids, list) or not pids or any(type(pid) is not int or pid <= 0 for pid in pids)
           for pids in (before, launched, observed)):
        raise ValueError('desktop process observations need nonempty lists of positive PIDs')
    if set(before) & set(launched) or not set(observed) <= set(launched):
        raise ValueError('the observation does not belong to a new app process')


def reopen_transition_ref(references):
    """Select the one transition receipt this attempt exports beside its counter samples."""
    matches = [ref for ref in references if isinstance(ref, str)
               and ref.rsplit('/', 1)[-1].endswith('-reopen-transition.json')]
    if len(matches) != 1:
        raise ValueError('reopen needs one quit and relaunch transition receipt')
    return matches[0]


def reopen_quit_call(call):
    """Recognize a successful structured Quit in the native event stream."""
    return call.get('tool') == 'brick_ui' and call.get('arguments') == {
        'target': 'farhelm', 'method': 'quit', 'args': []}


def require_reopen_transition(transition, identity, before, after):
    """Prove that this attempt's two exported counter samples span its owned desktop relaunch."""
    if transition['identity'] != identity:
        raise ValueError('reopen transition belongs to a different attempt or checkpoint')
    quit_witness = transition['quit']
    if quit_witness['requested'] is not True or quit_witness['desktop_pids'] != []:
        raise ValueError('reopen transition did not witness the app exit before launch')
    require_new_desktop_pids(transition['before']['desktop_pids'], transition['launched_desktop_pids'],
                             transition['after']['desktop_pids'])
    if transition['before']['sample'] != before or transition['after']['sample'] != after:
        raise ValueError('reopen transition does not match the exported counter samples')


# Each side of the pair has its own fixed observation of terminal hosts and of
# session rows; the old-session checkpoint for that side compares against the
# `old-sessions` receipts of the same operations.
SIDES = {
    'local': {'hosts': 'terminal_hosts', 'survival': 'host_survival', 'after': 'old_session_after',
              'state': 'inspect_state', 'old_check': 'local-old-session'},
    'remote': {'hosts': 'remote_terminal_hosts', 'survival': 'remote_host_survival',
               'after': 'remote_old_session_after', 'state': 'remote_inspect_state',
               'old_check': 'remote-old-session'},
}

# Checkpoints that need a floor under the operator's `--timeout`.
# The installer pair's `old-sessions` waits two genuine Codex conversations on
# the previous release through capture; each old-session check runs two agent
# turns around an agent Restart. Announcements needs time for both launches
# around the delivery window, including the normal relaunch when it expires.
TURN_FLOORS = {('old-sessions', 'p0-installer-upgrade'): 1800,
               ('local-old-session', 'p0-installer-upgrade'): 1800,
               ('remote-old-session', 'p0-installer-upgrade'): 1800,
               ('p2-announcements', 'p0-installer-upgrade'): 240}

# The standing fixture has a recognizable title so a compiled-in or live
# production announcement cannot satisfy the native check.
ANNOUNCEMENTS_TEST_PATH = 'announcements-test.json'
ANNOUNCEMENTS_TEST_TITLE = 'Farhelm brick test announcement'


def announcements_instructions():
    """Exercise the native bell with two quits around the one-off feed override."""
    return f'''Call announcements_feed first. If either file is unavailable, retain the
receipt and return its outcome with the exact detail. A 404 means the standing
test feed or signature is not published yet. Leave the running app alone when
the probe cannot establish publication.
Otherwise bind the running candidate with brick_ui. Use method quit with
args [] on target farhelm, then call launch_announcements.
That launch alone uses FARHELM_ANNOUNCEMENTS_PATH={ANNOUNCEMENTS_TEST_PATH}.
The staged launch leaves the pending batch for the bell even if it arrives
before the first window. If a launch operation returns not-ready after Quit,
observe the previous app's exit and repeat the operation.
Bind the new candidate and open the bell panel above Hosts. Reinspect the
panel for up to 60 seconds for an unresolved New announcement row with its
Open Announcements action. Other unread notices do not establish delivery.
If no matching row arrives, note what the panel showed and the elapsed wait.
Skip the dialog and history observations and go to the normal relaunch below.
If the row arrives, capture it and use its action to open Announcements. Require
the standing feed's title "{ANNOUNCEMENTS_TEST_TITLE}" and capture it.
Dismiss the dialog with Got it. Reopen the bell and verify the notice is
resolved with no Open Announcements action. Open ? > Announcements and capture
the same title in history. Only the standing feed's exact title counts.
In either case, finish the staged launch by calling brick_ui quit again on the
bound candidate, then call relaunch_announcements. It verifies
that the new process has no override. If no notice arrived,
report could-not-run with what the bell showed and the elapsed wait after the relaunch.
Pass needs the three native captures and the fixed command receipts.
Any P2 failure leaves the recorded P0, P1 and feedback results intact.'''


def turn_timeout(check, pass_id, requested):
    """Give a checkpoint at least its known floor, never more than the tester's maximum."""
    return min(3600, max(requested, TURN_FLOORS.get((check, pass_id), 0)))


def relaunch_checks_app_only(task):
    """Say whether this pass's relaunch judges the app alone: true for both P0 upgrade pairs.

Their local supervisor's upgrade, and any local Update action, belong to the
installer pair's `local-supervisor-upgrade`, so the relaunch proves a new app
process instead (the owned restart's transition receipt). Bonus upgrades keep
requiring the candidate supervisor at relaunch.
"""
    return task['pass_id'] in ('p0-installer-upgrade', 'p0-updater-upgrade')


def prerequisite_checks(contracts, check):
    """List the earlier checkpoints of a pass, in order, that `check` depends on.

`contracts` is the pass's declared checkpoint map, in order, each with its
priority. A P0 checkpoint depends on the earlier P0 ones only, so a failed
`old-sessions` (P1) cannot keep the candidate's supervisors from being tested;
P1 on every earlier P0 and P1 one, since the old sessions and the remote menu
need upgraded supervisors and share the same sessions; P2 on the earlier P0
ones; a bonus checkpoint on everything before it. An undeclared checkpoint
(native preflight) has none.
"""
    if check not in contracts:
        return []
    ordered = list(contracts)
    earlier = ordered[:ordered.index(check)]
    allowed = {'P0': ('P0',), 'P1': ('P0', 'P1'), 'P2': ('P0',)}.get(contracts[check].get('priority', 'bonus'))
    return earlier if allowed is None else [key for key in earlier
                                            if contracts[key].get('priority', 'bonus') in allowed]


def old_session_side(check):
    """Name the pair side an old-session checkpoint observes, or None for any other checkpoint."""
    return next((side for side, names in SIDES.items() if names['old_check'] == check), None)


def host_identity(host):
    """Key one observed terminal host by pid and platform start identity, never by pid alone.

The start identity is a string on the Mac and a dict on Linux; canonical JSON
compares both the same way across receipts.
"""
    return host['pid'], json.dumps(host['start'], sort_keys=True)


def require_old_session(observations, side):
    """Refuse an old-session pass whose machine-owned samples contradict it.

The survival comparison must hold: the old session keeps its id and launch
generation across the upgrade, and its terminal host and that host's child are
the same processes as at the old fixture. The final sample, taken after the
session's Restart, must show a later generation with a captured conversation,
exactly one terminal host that appeared since the survival sample, and the old
fixture's host gone. A final sample taken before the Restart therefore refuses
rather than passes. Whether the resumed agent remembers the earlier turn stays
the UI's to report.
"""
    names = SIDES[side]
    survival = observations[names['survival']]
    after = observations[names['after']]
    if survival['survived'] is not True:
        raise ValueError('the old session or its terminal host did not survive the upgrade')
    old, row = survival['old_session'], after['session']
    if row is None:
        raise ValueError('the old session is gone after the check')
    if type(row.get('generation')) is not int or type(old.get('generation')) is not int:
        raise ValueError('the old session has no comparable launch generation')
    if row['generation'] <= old['generation'] or not row.get('captured_conversation'):
        raise ValueError('the old session restart and resume premise was not established')
    if len(after['appeared_hosts']) != 1:
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
        # Each supervisor's upgrade is observed on its own before an agent runs
        # on it; the agent checks repeat the version observation they rely on.
        'local-supervisor-upgrade': {'versions'}, 'remote-supervisor-upgrade': {'remote_version'},
        'local-supervisor-agent': {'versions'},
        'remote-supervisor-agent': {'remote_version'},
        # One old session per host on the previous release: its session row,
        # captured conversation and terminal identities are the baseline the
        # later old-session checks compare against.
        'old-sessions': {'versions', 'inspect_state', 'remote_inspect_state', 'terminal_hosts',
                         'remote_terminal_hosts'},
        # The survival comparison samples before the session is touched and runs
        # once; the final sample is refused unless the comparison ran first.
        'local-old-session': {'prior_receipts', 'host_survival', 'old_session_after'},
        'remote-old-session': {'prior_receipts', 'remote_host_survival', 'remote_old_session_after'},
        'remote-host-remove': {'remote_host_before', 'remote_host_after', 'remote_version'},
        'remote-host-uninstall': {'remote_host_before', 'remote_host_after'},
        'p2-announcements': {'announcements_feed', 'launch_announcements', 'relaunch_announcements'},
        'remote-provision': {'remote_version'}, 'codex-conversation': {'inspect_state'},
        'seed-state': {'inspect_state', 'versions'},
        'continuity-before': {'continuity_before'}, 'preserved-state': {'inspect_state'},
        'window-drag': {'prior_receipts', 'window_frame'},
        'codex-resume': {'inspect_state'},
        'remote-update': {'remote_version', 'continuity_remote_after'},
        'reopen': {'launch', 'versions', 'remote_version', 'continuity_reopen_before', 'continuity_reopen_after'}}
    result = set(requirements.get(check, {'prior_receipts'}))
    # The updater pair checks discovery and relaunch only; every other old
    # fixture provisions the remote host its later checks upgrade.
    if check == 'old-fixture' and pass_id != 'p0-updater-upgrade':
        result.add('remote_version')
    if check == 'update-relaunch' and update_path == 'manual':
        result.add('install_candidate')
    if check == 'update-relaunch' and update_path == 'override':
        result.add('public_latest')
    if update_path == 'override' and (check == 'update-discovery' or (
            check == 'update-relaunch' and pass_id.startswith('upgrade-'))):
        result.add('launch_override')
    return result


class EffectFailure(RuntimeError):
    """The machine-owned snapshots contradict a remote menu action that started from a valid premise.

Unlike a failed premise (no provisioned host to start from), this is the
product's own result: data lost, an uninstall that left things running, or a
Remove that changed the host. The import records it as a product failure.
"""


def _resources_preserved_by_remove(snapshot):
    """Compare the managed host across closure of the helm's SSH connection.

The remote stdio proxy belongs to that connection. Its exit is compatible
with an unchanged supervisor, terminal hosts and other remote resources.
Earlier snapshots have no proxy classification and compare every PID.
"""
    resources = dict(snapshot)
    proxy_pids = resources.pop('stdio_proxy_pids', ())
    resources['farhelm_pids'] = set(resources['farhelm_pids']).difference(proxy_pids)
    return resources


def require_remote_host_start(before, pass_id=None):
    """Prove that the observed starting installation can test the menu effect.

The peer must have a running supervisor in the process scan and saved session
IDs in its readable database. An empty baseline cannot prove data retention.
"""
    service = before['service']
    if pass_id == REMOTE_MAC_PASS:
        if service['kind'] != 'launchd':
            raise RuntimeError('the remote Mac check requires launchd observations')
        pid = service['pid']
        running = service['loaded'] is True and service['state'] == 'running' and type(pid) is int and pid > 0
    else:
        pid = int(service['MainPID']) if service['MainPID'].isdigit() else 0
        running = service['ActiveState'] == 'active' and service['SubState'] == 'running' and pid > 0
    if (not running or pid not in before['farhelm_pids'] or not before['session_ids']
            or not all(before[key] for key in ('lib_present', 'unit_present', 'binary_sha256',
                                              'unit_sha256', 'data_present', 'database_present'))):
        raise RuntimeError('remote menu check did not start from a provisioned running host with readable data')


def require_remote_host_effect(check, before, after, pass_id=None):
    """Distinguish forgetting a host from uninstalling its managed resources.

Remove must preserve the host while closing its own SSH connection. Uninstall
must leave the same saved sessions and remove the service, installed files
and processes. Contradictions after a valid start are product failures.
Native images independently prove the menu and registry behavior.
"""
    if check not in ('remote-host-remove', 'remote-host-uninstall'):
        raise ValueError('not a remote host menu checkpoint')
    require_remote_host_start(before, pass_id)
    # Kept data means the same data: an emptied, replaced or unreadable
    # database keeps the file but loses the sessions it held.
    if not after['data_present'] or not after['database_present'] or after['session_ids'] != before['session_ids']:
        raise EffectFailure('remote menu action lost the retained data directory or database')
    if check == 'remote-host-remove':
        if _resources_preserved_by_remove(before) != _resources_preserved_by_remove(after):
            raise EffectFailure('Remove changed remote resources or the running supervisor')
        return
    service = after['service']
    if pass_id == REMOTE_MAC_PASS:
        if service['kind'] != 'launchd':
            raise RuntimeError('the remote Mac check requires launchd observations')
        stopped = service['loaded'] is False and service['pid'] is None
    else:
        stopped = (service['MainPID'] == '0' and service['ActiveState'] == 'inactive'
                   and service['LoadState'] == 'not-found')
    if (after['lib_present'] or after['unit_present'] or after['binary_sha256'] or after['unit_sha256']
            or not stopped or after['farhelm_pids']):
        raise EffectFailure('Uninstall left remote binaries, unit, supervisor or terminal hosts running')


def available_operations(check, pass_id, update_path):
    """Offer only this task's mutations plus bounded independent observations.

Read-only observations can help diagnose an unexpected UI state. Installers,
launches and continuity milestones belong only to their declared checkpoint.
"""
    result = required_operations(check, pass_id, update_path) | {
        'prior_receipts', 'versions', 'remote_version', 'update_logs', 'inspect_state',
        'remote_inspect_state', 'terminal_hosts', 'remote_terminal_hosts'}
    if pass_id == REMOTE_MAC_PASS:
        # The Linux terminal-host probe reads procfs. The remote Mac uses the
        # fixed launchd and process snapshots in the uninstall checkpoint.
        result.discard('remote_terminal_hosts')
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
        discovery = '''Bind the old app and use brick_ui method quit with args [].
Wait for versions to show that the old app exited, then call launch_override.
Call update_logs and require the startup override announcement.\n''' + discovery
    click = '''Wait for the actual Restart to update offer. Use brick_ui method restart with
coordinates from a fresh native image of that menu: args [[x,y]]. The harness
owns the single native click and process handoff.
Require the candidate returns usable.
'''
    end = '''Never manually launch or use getApp to rescue a missing restart. Call versions
after the candidate returns. Capture the final native acceptance state. The
candidate may update the remote supervisor during relaunch. Its later
checkpoint verifies the outcome.'''
    if relaunch_checks_app_only({'pass_id': pass_id}):
        # P0 judges the app here; the local supervisor's upgrade is a P0
        # checkpoint of its own on the installer pair.
        supervisor = '''This relaunch judges the app alone. Whether the local supervisor upgraded, and
any local Update action, are checked by P0's local-supervisor-upgrade
checkpoint (on the installer pair): do not wait for it or use Update here, and
never stop a supervisor yourself.
'''
    else:
        supervisor = '''Observe local connection. An old app that runs its supervisor detached leaves
it running when it quits, and the candidate replaces it on first launch, so a
brief connecting state is the handover: wait for the connected candidate
local supervisor, never stop one yourself.
'''
    restart = click + supervisor + end
    if check == 'update-discovery':
        if update_path == 'manual':
            raise ValueError('manual installation cannot satisfy updater discovery')
        return 'Call prior_receipts for established premises.\n' + discovery + '\nDo not restart in this checkpoint.'
    if update_path == 'manual':
        return '''Call prior_receipts. Call install_candidate while the old app runs.
After installation the old app can take about a minute to notice the changed
installed version. Sample fresh native state until the version readout turns
red or its current accessibility label explicitly offers Restart to update.
An unchanged Check for updates readout means keep observing. When the restart
offer appears, open its menu by clicking the readout itself at coordinates
from a fresh native screenshot. The accessibility ID can expire while the
readout changes. Capture the visible menu before proceeding.
''' + restart
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


def remote_host_instructions(check, pass_id=None):
    """Exercise the remote menu before feedback, using the agent sessions P0 and the old-session checks left running.

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
    if check == 'remote-host-uninstall' and pass_id == REMOTE_MAC_PASS:
        return '''Call prior_receipts for the connected candidate on context.ssh.alias.
Through Farhelm's native New session launcher, choose that Mac host and start one plain-command
session (agent type none, /bin/sh). Verify it starts on the remote host, then
Stop it through Farhelm and confirm it has exited. Keep its session data, and
leave the local host untouched. Use remote_inspect_state to note the remote
session ID. Call remote_host_before while the remote supervisor still runs.
Require the fixed snapshot to contain that session ID and a running launchd job.
Open that remote host's menu and choose Uninstall. Inspect the actual plan
before confirming. Require it to remove the launchd job, its plist and the
private lib directory, and to keep the Farhelm data directory. Retain a native
image of the plan, then choose its ordinary Uninstall confirmation once.
Require the remote host and cached sessions to disappear, and the kept-data
removal notice to appear. Call remote_host_after. Require the job unloaded,
its plist and lib directory gone, and no Farhelm process left. Require the
original data directory, database and session ID. Retain a native image of
completion. The local host and candidate app must still work. Do not re-add,
reinstall or substitute direct service or file mutations for the menu action.'''
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
running supervisor, no Farhelm process left at all (farhelm_pids empty: no
terminal host either), and the original data directory/database still present.
Retain a final native image of completion and the usable candidate app/local
host. Do not re-add, reinstall, retry an uncertain uninstall or remove files to
manufacture success. P2 feedback follows on this still-running Mac candidate.'''
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
