//! Shared session-list state and host-selection rules.
//!
//! These types keep the view, row, and create form on one representation of
//! row state and host identity.

use crate::hosts::{host_incarnation, is_connected, phase_label};
use crate::peer::display_peer;
use crate::{Host, HostId, HostKind, Session, SessionStatus};

/// The subset of `Session` `view::ListView`'s `on_delete` actually needs:
/// the id the API call targets, plus `status` to decide whether this click
/// deletes immediately or opens the inline confirm prompt. No `title` —
/// since the old eval-based `window.confirm()` was replaced with
/// `row::SessionRow`'s in-page prompt, the row derives its wording from the
/// live `session.title` and `session.status` on every render. `on_delete`
/// never builds that wording, so a title never needs to travel through this
/// type at all. Deliberately narrower than the whole `Session` — `on_delete`
/// has no legitimate reason to depend on `cwd` or `invocation` either, and a
/// dedicated type is what makes that impossible rather than merely
/// unlikely, unlike `on_open` (which keeps taking the whole `Session`: it
/// needs every field to populate `SessionView`).
#[derive(Debug, Clone)]
pub(super) struct DeleteTarget {
    pub(super) id: String,
    pub(super) status: SessionStatus,
}

/// The display state `ListView` derives for one `SessionRow` render.
///
/// None of these values owns list behavior. Keeping them together makes
/// that boundary explicit: the row decides how to present the current
/// snapshot, while `ListView` remains the owner of every signal and lock
/// the snapshot came from.
#[derive(Clone, PartialEq)]
pub(super) struct RowState {
    pub(super) error: Option<String>,
    pub(super) busy: bool,
    pub(super) confirming: bool,
    pub(super) confirming_archive: bool,
    pub(super) renaming: bool,
    pub(super) nav_disabled: bool,
    /// Whether this row's actions menu is the (at most one) open one —
    /// the hosts panel's `profiles_open` shape, derived from `ListView`'s
    /// `menu_open` signal.
    pub(super) menu_open: bool,
    /// Whether this row's session is the one the right pane currently
    /// shows — derived from `ListView`'s `selected` signal, same shape as
    /// `menu_open`, and grouped here because it is row display state,
    /// which is this struct's whole charter. Memoization does not require
    /// the grouping — a directly-compared bool prop would behave the same
    /// — it only requires the value to stay part of the compared props,
    /// which is what keeps a selection switch down to two row renders
    /// (see the render-count tests in `row::tests`).
    pub(super) selected: bool,
    /// Whether this row's session runs on the helm's OWN machine, which is
    /// what decides whether the row spends a line naming its host at all
    /// (see [`session_is_local`] for why the row cannot answer this itself).
    ///
    /// Part of `RowState` rather than a prop of its own for the reason the
    /// whole struct exists: it is derived display state, and the row's
    /// memoization only compares what is in the compared props. A fleet
    /// where every session is local therefore renders no host lines and
    /// still re-renders exactly the rows a selection change touched.
    pub(super) host_is_local: bool,
}

/// Whether a session sits on the helm's own machine, given the registry's
/// local row id.
///
/// The row itself cannot answer this: a `Session` carries a host id and the
/// helm's rendering of that host's NAME, but nothing that says which kind of
/// registry row it is — `HostKind::Local` lives on the `Host`, which only
/// `ListView` reads.
///
/// Comparing ids rather than names is deliberate. The name is a DISPLAY
/// string the helm derives for humans (`host_display_name`: the words "this
/// machine" for the local row, the ssh destination otherwise), so matching
/// on it would tie a structural decision to a wording choice — a reworded
/// local row, or an ssh destination that happened to render the same
/// string, would silently drop the host line off sessions on another
/// machine. The id has no such failure mode.
///
/// Both absences answer FALSE, which is the honest direction rather than
/// the pretty one. A session with no `host` came from a helm too old to
/// send one, and a `None` local id means the hosts read has not landed yet
/// (or failed) — in neither case is there evidence the session is local.
/// Put precisely: unknown locality never SUPPRESSES an available host
/// label — this function only ever pushes the row toward showing a host
/// name it already has, never away from it. Whether a name actually
/// appears is a separate question the row answers on its own: a legacy row
/// with no `host_name` at all necessarily shows none, this function's
/// verdict notwithstanding. The visible cost, for a row that does carry a
/// name, is a host line that disappears on a fresh page load once
/// `/api/hosts` answers; the alternative is a row that silently claims a
/// remote session is local while the registry is still unknown, and that
/// one is a lie rather than a flicker.
pub(super) fn session_is_local(session_host: Option<HostId>, local_host: Option<HostId>) -> bool {
    session_host.is_some() && session_host == local_host
}

/// One entry in the create dialog's host selector: everything that decides
/// how a host is offered, and nothing more.
///
/// Narrower than `Host` on purpose — the dialog has no business re-deriving
/// anything from a `HostPhase`, which is the duplication `hosts`'s helpers
/// exist to prevent. `ListView` reduces each host to these facts once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostOption {
    pub(super) id: HostId,
    /// The helm's own display name, so the selector and the session rows
    /// call a host the same thing.
    pub(super) name: String,
    /// Whether this is the helm's own machine — the reserved local row,
    /// which is SPEC.md's fallback default.
    pub(super) local: bool,
    /// The phase to show beside the name, for a host that is NOT connected;
    /// `None` for one that is.
    ///
    /// Every host is offered as a create target regardless of phase (see
    /// `ListView`), so the label is what keeps that from being a trap: a
    /// user picking a host the helm will refuse can see why before they
    /// submit, and the refusal that follows repeats the same word.
    pub(super) phase: Option<String>,
    /// This host's current incarnation (`hosts::host_incarnation`) — what an
    /// idempotency key is bound to, so that an id pointed at a different
    /// machine since the key was minted is a different intent.
    pub(super) incarnation: String,
    /// The helm's CONNECTION token for this host (`Host::incarnation`), which
    /// a create hands back as its `expected_incarnation` so the helm can
    /// refuse one prepared against an install that has since been replaced.
    /// Distinct from the fingerprint above: that one describes the registry
    /// ROW, this one the live connection behind it.
    pub(super) connection: u64,
    /// The registry's recorded install identity (`Host::identity`), the
    /// third identity notion here and the one the create DEFAULT compares
    /// (see [`default_create_host`]): unlike the fingerprint it survives a
    /// pure address retarget of the same machine, and unlike the connection
    /// token it survives a reconnect — it changes exactly when the install
    /// behind the row does, which is the only change that should evict a
    /// default derived from "the session the user is looking at".
    pub(super) identity: Option<String>,
    /// Whether the host sits in the identity-mismatch phase: the endpoint
    /// reported an identity DIFFERENT from the recorded one and the helm
    /// froze awaiting adopt-or-fix. `identity` above still holds the
    /// RECORDED (predecessor) value in that phase, so an identity
    /// comparison alone would keep matching a row known to front a
    /// different install — this flag is what lets the create default
    /// disqualify it instead (see [`default_create_host`]).
    pub(super) identity_mismatch: bool,
}

impl HostOption {
    /// What the `<option>` reads: the host's name, with its phase appended
    /// when there is one to warn about.
    ///
    /// The name is [`display_peer`]'d for the reason every other rendering
    /// of a destination is — an option label is exactly the place where a
    /// directional override could make one host's row read as another's, and
    /// the choice made there is which machine a command runs on.
    pub(super) fn label(&self) -> String {
        let name = display_peer(&self.name);
        match &self.phase {
            Some(phase) => format!("{name} ({phase})"),
            None => name,
        }
    }
}

/// The selected session's host as the create default's input: the row id
/// the session row named, plus the install identity it named alongside it.
///
/// Snapshotted from the `Session` at selection time (see `AppBody`'s
/// `open_host` wiring), so `identity` says which INSTALL the user was
/// looking at when they selected — which is exactly what
/// [`default_create_host`] compares against the registry as it stands at
/// create time. `identity`'s two `None` layers keep the wire's two
/// absences apart; see `Session::host_identity` for the contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenHost {
    pub(crate) id: HostId,
    pub(crate) identity: Option<Option<String>>,
}

impl OpenHost {
    /// The selected session's host binding — id AND the install identity
    /// the row reported, snapshotted together at selection time.
    ///
    /// This is THE production adapter from a `Session` to the create
    /// default's input: `AppBody` calls it on the selected session, and
    /// [`default_create_host`] compares what it produces. It exists as a
    /// named function (rather than an inline struct literal at the call
    /// site) so a test can pin that the identity actually rides along — an
    /// adapter that dropped it would silently restore the row-id-only
    /// behavior while every comparison-level test stayed green. `None` for
    /// a session that names no host (a helm too old to send `host` has
    /// nothing to bind a default to).
    pub(crate) fn of_session(session: &Session) -> Option<OpenHost> {
        Some(OpenHost {
            id: session.host?,
            identity: session.host_identity.clone(),
        })
    }
}

/// Which host a fresh create dialog should target: the LOCAL row, whatever
/// phase it is in.
///
/// ## Why SPEC.md's first clause is not implemented here
///
/// SPEC.md's default is "the host of the currently open session, else the
/// helm's own host". Both clauses are live: the sidebar layout
/// (BUGS_BURNDOWN.md issue 5) put the create form beside an open session,
/// which is exactly the coexistence an earlier version of this function
/// documented as the condition for restoring the first clause. `open_host`
/// is the SELECTED session's host — the one open right now, never a
/// remembered last-viewed one, because a session the user deselected is
/// not open — and it only wins while that host is still in the registry
/// AND still the install the session row reported (an open session whose
/// host was removed must not aim a create at a selector entry that no
/// longer exists, and one whose host row was retargeted or adopted onto a
/// DIFFERENT install must not aim a create at the successor — the row id
/// survives both edits, which is exactly why the id alone was the
/// #156-review residual this check closes).
///
/// The install comparison: a row whose `identity` is the outer `None`
/// came from a helm that predates `host_identity`, and gets the old
/// row-id-only behavior — degrading, not failing, is what keeps a new UI
/// working against an old helm. Otherwise the session row's recorded
/// identity must equal the registry's current one, where `None == None`
/// (identity-less then, identity-less now) deliberately passes. That is a
/// KNOWN residual, accepted rather than closable: an identity-less host is
/// one whose install has never identified itself to the registry (see the
/// helm store's first-contact and retarget docs), so two absent identities
/// carry no continuity evidence in either direction — and failing the
/// comparison on "no evidence" would break the default for every
/// never-probed host, trading a rare wrong-default window on identity-less
/// installs for a broken default on all of them. A host frozen in the
/// identity-mismatch phase is disqualified outright: its recorded identity
/// still names the predecessor while the endpoint already reports a
/// different install, so an identity match there is precisely the false
/// positive this function exists to refuse.
///
/// ## Two things the fallback deliberately does not do
///
/// - It does not skip a local row that is not connected. SPEC.md names the
///   helm's own host unconditionally; a create against it is a precondition
///   failure the helm explains in its own words, and a default that quietly
///   moved to another machine because the local supervisor was down would be
///   a create on a host the user never chose.
/// - It does not fall back to "the first host in the list" when there is no
///   local row. The first host is whichever the registry happens to order
///   first, and creating on it is a guess. `None` — a selector with nothing
///   chosen, which the form refuses to submit — is the honest answer, and it
///   only arises before the first hosts read lands, since a live helm always
///   has its local row.
fn default_create_host(hosts: &[HostOption], open_host: Option<&OpenHost>) -> Option<HostId> {
    open_host
        .filter(|open| {
            hosts.iter().any(|host| {
                // An outer-None `open.identity` is a row from a helm that
                // predates the field: no identity claim at all, so the id
                // is the only comparable fact and it alone decides.
                host.id == open.id
                    && !host.identity_mismatch
                    && open
                        .identity
                        .as_ref()
                        .is_none_or(|seen| *seen == host.identity)
            })
        })
        .map(|open| open.id)
        .or_else(|| hosts.iter().find(|host| host.local).map(|host| host.id))
}

/// The host a create would ACTUALLY go to: the user's choice while it still
/// exists, and [`default_create_host`]'s answer otherwise.
///
/// Extracted because two places need the same answer and a second copy would
/// be the one that drifted: the create dialog renders it (a selector must
/// show the target that would be used, never one that has since left the
/// registry) and `ListView` points the profile catalog's reader at it. A
/// disagreement between those two would put one host's profiles under another
/// host's create — the exact failure `profiles::CatalogRead` refuses to make
/// possible from its side.
pub(super) fn effective_create_host(
    hosts: &[HostOption],
    chosen: Option<HostId>,
    open_host: Option<&OpenHost>,
) -> Option<HostId> {
    chosen
        .filter(|chosen| hosts.iter().any(|host| host.id == *chosen))
        .or_else(|| default_create_host(hosts, open_host))
}

/// Fill the helm-owned host fields a create reply deliberately omits.
///
/// `POST /api/sessions` answers with bare `SessionInfo` — no host, no
/// identity — because the caller already knows which host it asked for.
/// This is where that knowledge is applied: `host` is the submitted row and
/// `identity` the submitted option's registry identity, so the created
/// session can stand in as the selected row (naming the right host, holding
/// the create default) before the first authoritative detail read answers.
/// Removing this enrichment would not break any listing — it would only
/// reopen the wrong-default window for exactly the interval between a
/// create and its first refresh, which is why it has its own test.
///
/// Known residual, accepted: the identity is the form's snapshot, and a
/// host that learns its identity between that snapshot and the create
/// succeeding gets stamped a stale `Some(None)` here. The skew fails
/// toward MISMATCH — the next default falls back locally instead of
/// following the row — and the first detail refresh replaces the stamp, so
/// the cost is a briefly conservative default, not a wrong-machine create.
pub(super) fn enrich_created_session(
    reply: Session,
    host: HostId,
    identity: Option<Option<String>>,
) -> Session {
    Session {
        host: reply.host.or(Some(host)),
        host_identity: reply.host_identity.or(identity),
        ..reply
    }
}

/// Every registered host as the create dialog and the filter surface offer
/// it.
///
/// A free function over the snapshot rather than an inline `map` in the
/// render, because `ListView`'s target effect needs exactly the same reduction
/// from inside an effect, where the render's local is not in scope. Every host
/// is offered whatever phase it is in — see `ListView` for why filtering to
/// connected ones would quietly rewrite SPEC.md's creation default.
pub(super) fn host_options(hosts: &[Host]) -> Vec<HostOption> {
    hosts
        .iter()
        .map(|host| HostOption {
            id: host.id,
            name: host.name.clone(),
            local: host.kind == HostKind::Local,
            // Non-connected hosts are labelled with their phase, so choosing
            // one is an informed choice rather than a surprise refusal.
            phase: (!is_connected(&host.state)).then(|| phase_label(&host.state).to_string()),
            incarnation: host_incarnation(host),
            connection: host.incarnation,
            identity: host.identity.clone(),
            identity_mismatch: matches!(host.state, crate::HostPhase::IdentityMismatch { .. }),
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::row::row_specimen;
    use super::*;

    /// A CONNECTED host as the create dialog would be offered it —
    /// `phase: None` is what "connected" means to an option.
    pub(in super::super) fn option(id: HostId, name: &str, local: bool) -> HostOption {
        HostOption {
            id,
            name: name.into(),
            local,
            phase: None,
            incarnation: format!("incarnation-{id}"),
            connection: 1,
            identity: Some(format!("install-{id}")),
            identity_mismatch: false,
        }
    }

    /// The selected session's host as a row from a CURRENT helm would
    /// report it: id plus the same install identity [`option`] gives that
    /// row, i.e. selection and registry agree.
    pub(in super::super) fn open(id: HostId) -> OpenHost {
        OpenHost {
            id,
            identity: Some(Some(format!("install-{id}"))),
        }
    }

    /// The same, for a host in some non-connected phase: still offered, and
    /// labelled with the phase so the choice is informed.
    pub(in super::super) fn option_in(
        id: HostId,
        name: &str,
        local: bool,
        phase: &str,
    ) -> HostOption {
        HostOption {
            phase: Some(phase.into()),
            ..option(id, name, local)
        }
    }

    /// With nothing selected, the default is the LOCAL row — SPEC.md's
    /// second clause, the one that fires whenever the first cannot.
    #[test]
    fn the_create_default_is_the_local_row() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
        ];
        assert_eq!(default_create_host(&hosts, None), Some(1));
    }

    /// The SELECTED session's host wins over the local row while it is in
    /// the registry, and falls back the moment it is not — SPEC.md's first
    /// create-default clause, live again now that the sidebar layout lets
    /// the create form and an open session coexist (BUGS_BURNDOWN.md
    /// issue 5).
    #[test]
    fn the_open_sessions_host_wins_while_it_exists() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
        ];
        assert_eq!(default_create_host(&hosts, Some(&open(2))), Some(2));
        assert_eq!(
            default_create_host(&hosts, Some(&open(99))),
            Some(1),
            "a selected session whose host left the registry falls back to the helm's own host"
        );
    }

    /// The first clause is bound to the INSTALL, not the row id: a row id
    /// survives retargets and adoptions while the machine behind it
    /// changes, so a selection made against one install must not default a
    /// create onto its successor (the #156-review residual).
    ///
    /// The install-comparison cases, one per line of the contract in
    /// [`default_create_host`]'s doc (the agreeing case — identity match
    /// keeps the default — is already pinned by
    /// `the_open_sessions_host_wins_while_it_exists` above): disagreement
    /// falls back to the local row, in BOTH directions — an identity
    /// appearing where the row reported none, and an identity vanishing
    /// where the row reported one, are transitions whose continuity cannot
    /// be verified, so the default falls back rather than guessing; and a
    /// session from a helm that predates `host_identity` (outer `None`)
    /// degrades to the row-id-only behavior rather than losing the
    /// default.
    #[test]
    fn the_open_sessions_host_wins_only_while_it_is_the_same_install() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
        ];
        let retargeted = OpenHost {
            id: 2,
            identity: Some(Some("install-superseded".to_string())),
        };
        assert_eq!(
            default_create_host(&hosts, Some(&retargeted)),
            Some(1),
            "a row id now naming a different install must not aim the default at the successor"
        );

        let was_identityless = OpenHost {
            id: 2,
            identity: Some(None),
        };
        assert_eq!(
            default_create_host(&hosts, Some(&was_identityless)),
            Some(1),
            "an identity appearing on a previously identity-less row cannot be verified as the \
             same install, so the default falls back rather than guessing"
        );

        let identity_cleared = OpenHost {
            id: 2,
            identity: Some(Some("install-2".to_string())),
        };
        let now_identityless = vec![
            option(1, "this machine", true),
            HostOption {
                identity: None,
                ..option(2, "user@box", false)
            },
        ];
        assert_eq!(
            default_create_host(&now_identityless, Some(&identity_cleared)),
            Some(1),
            "a row that reported an identity while the registry now records none is the same \
             unverifiable transition in reverse — treating every currently identity-less row \
             as compatible would wave it through"
        );

        let old_helm = OpenHost {
            id: 2,
            identity: None,
        };
        assert_eq!(
            default_create_host(&hosts, Some(&old_helm)),
            Some(2),
            "a helm that predates host_identity keeps the old row-id behavior instead of \
             dropping the default"
        );
    }

    /// Identity-less rows from a CURRENT helm still hold the default while
    /// the registry stays identity-less: `None == None` passes on purpose.
    ///
    /// Why the exception is retained: an identity-less host has never
    /// identified itself to the registry, so two absent identities carry no
    /// continuity evidence in either direction — there is nothing to
    /// compare, not a comparison that passed. Failing on "no evidence"
    /// would break the default for every never-probed host; the price is
    /// the narrow residual that an identity-less install replaced by
    /// another identity-less install goes unnoticed (documented on
    /// [`default_create_host`] and in SPEC_impl.md's creation-default
    /// note).
    #[test]
    fn an_identityless_row_matching_an_identityless_registry_keeps_the_default() {
        let identityless = HostOption {
            identity: None,
            ..option(2, "user@box", false)
        };
        let hosts = vec![option(1, "this machine", true), identityless];
        let selection = OpenHost {
            id: 2,
            identity: Some(None),
        };
        assert_eq!(default_create_host(&hosts, Some(&selection)), Some(2));
    }

    /// A create reply is bare `SessionInfo`; the enrichment is what makes
    /// the created session able to hold the create default before its
    /// first detail refresh.
    ///
    /// Why this exists: removing [`enrich_created_session`] (or dropping
    /// the identity half of it) breaks no listing and no other test — it
    /// only reopens the wrong-default window for the interval between a
    /// create and its first refresh. Both halves are pinned: the submitted
    /// host and identity land on a bare reply, and a reply that somehow
    /// carried its own values keeps them (`or`, not overwrite).
    #[test]
    fn a_created_session_is_stamped_with_the_submitted_host_and_identity() {
        let bare = row_specimen("made");
        let enriched = enrich_created_session(bare, 7, Some(Some("install-7".to_string())));
        assert_eq!(enriched.host, Some(7));
        assert_eq!(enriched.host_identity, Some(Some("install-7".to_string())));

        let mut already = row_specimen("made");
        already.host = Some(3);
        already.host_identity = Some(Some("install-3".to_string()));
        let kept = enrich_created_session(already, 7, Some(Some("install-7".to_string())));
        assert_eq!(
            (kept.host, kept.host_identity),
            (Some(3), Some(Some("install-3".to_string()))),
            "a reply that names its own host fields is authoritative over the stamp"
        );
    }

    /// The two production adapters feeding [`default_create_host`] must
    /// both carry the install identity through — dropping it in either one
    /// restores the row-id-only behavior while every comparison-level test
    /// above stays green, which is exactly how the #156 residual would
    /// quietly come back.
    ///
    /// [`OpenHost::of_session`] is what `AppBody` calls on the selected
    /// session; [`host_options`] is what `ListView` reduces the host
    /// catalog through. Each is asserted on the fields the comparison
    /// reads: id + identity (+ the identity-mismatch disqualifier on the
    /// catalog side).
    #[test]
    fn the_identity_survives_both_adapters_into_the_comparison() {
        let mut session = row_specimen("sess");
        session.host = Some(7);
        session.host_identity = Some(Some("install-7".to_string()));
        let open = OpenHost::of_session(&session).expect("a hosted session adapts");
        assert_eq!(open.id, 7);
        assert_eq!(open.identity, Some(Some("install-7".to_string())));

        let mut hostless = row_specimen("sess");
        hostless.host = None;
        assert_eq!(
            OpenHost::of_session(&hostless),
            None,
            "a session naming no host binds no default"
        );

        let catalog = vec![Host {
            id: 7,
            kind: HostKind::Ssh,
            destination: Some("user@box".to_string()),
            name: "user@box".to_string(),
            identity: Some("install-7".to_string()),
            remote_farhelm: None,
            remote_state_dir: None,
            state: crate::HostPhase::IdentityMismatch {
                recorded: "install-7".to_string(),
                reported: "install-8".to_string(),
            },
            incarnation: 3,
        }];
        let options = host_options(&catalog);
        assert_eq!(options[0].id, 7);
        assert_eq!(
            options[0].identity,
            Some("install-7".to_string()),
            "the catalog adapter carries the registry identity"
        );
        assert!(
            options[0].identity_mismatch,
            "and surfaces the mismatch phase the comparison must disqualify on"
        );
    }

    /// A host frozen in the identity-mismatch phase loses the default even
    /// though its RECORDED identity still equals the one the session row
    /// reported.
    ///
    /// Why this matters: in that phase the registry's identity names the
    /// predecessor while the endpoint already reports a different install,
    /// so the recorded-identity comparison alone is a false positive — the
    /// one shape of "same identity, different machine" the comparison
    /// cannot see. SPEC.md's default must fall back locally rather than
    /// aim a create at a row the helm would refuse anyway.
    #[test]
    fn an_identity_mismatched_host_loses_the_default_despite_matching_identity() {
        let hosts = vec![
            option(1, "this machine", true),
            HostOption {
                identity_mismatch: true,
                phase: Some("identity-mismatch".to_string()),
                ..option(2, "user@box", false)
            },
        ];
        assert_eq!(
            default_create_host(&hosts, Some(&open(2))),
            Some(1),
            "a row known to front a different install than it records must not hold the default"
        );
    }

    /// The local row is the fallback WHATEVER phase it is in — SPEC.md names
    /// the helm's own host unconditionally.
    ///
    /// This is the rule an earlier shape got wrong by filtering the options
    /// to connected hosts: with the local supervisor down, the local row
    /// vanished from the list and the default silently moved to a remote
    /// machine, so a create the user thought was landing here would launch
    /// an agent somewhere else. Offering it and letting the helm refuse in
    /// its own words is the honest behavior.
    #[test]
    fn the_local_row_is_the_default_even_when_it_is_not_connected() {
        let hosts = vec![
            option_in(1, "this machine", true, "unreachable-reprobing"),
            option(2, "user@box", false),
        ];
        assert_eq!(
            default_create_host(&hosts, None),
            Some(1),
            "a down local host is still the helm's own host"
        );
    }

    /// With no local row to fall back on, the answer is NOTHING rather than
    /// an arbitrary host.
    ///
    /// "The first host in the list" was the earlier fallback and had no rule
    /// behind it: first is whatever the registry happens to order first, and
    /// creating an agent there is a guess about which machine the user
    /// meant. `None` leaves the selector empty, which the form refuses to
    /// submit with a reason. In practice this state exists only before the
    /// first hosts read lands, since a live helm always has its local row.
    #[test]
    fn a_fleet_with_no_local_row_defaults_to_nothing_rather_than_guessing() {
        assert_eq!(default_create_host(&[], None), None);

        let remote_only = vec![option(2, "user@box", false), option(3, "user@other", false)];
        assert_eq!(
            default_create_host(&remote_only, None),
            None,
            "picking whichever host sorted first would be a create on a machine nobody chose"
        );
    }

    /// A non-connected option must SAY so in its label, and a connected one
    /// must not be decorated.
    ///
    /// Every host is selectable now, so the label is what keeps that from
    /// being a trap: the phase a user sees before choosing is the same word
    /// the helm's refusal will use if they choose it anyway.
    #[test]
    fn an_option_label_names_the_phase_only_when_there_is_one_to_warn_about() {
        assert_eq!(option(1, "this machine", true).label(), "this machine");
        assert_eq!(
            option_in(2, "user@box", false, "unreachable-reprobing").label(),
            "user@box (unreachable-reprobing)"
        );
        // The name is peer/user-supplied and is escaped like every other
        // rendering of it: an option label is exactly where a directional
        // override could make one host read as another, and what is chosen
        // there decides which machine a command runs on.
        assert_eq!(
            option(3, "user@\u{202E}box", false).label(),
            "user@<U+202E>box"
        );
    }

    /// One registry row as `host_options` sees it: only `id` and `kind`
    /// matter for deriving `HostOption::local`, and every other field is
    /// filler kept out of the test below.
    fn registry_row(id: HostId, kind: HostKind) -> Host {
        Host {
            id,
            kind,
            destination: (kind == HostKind::Ssh).then(|| format!("user@box-{id}")),
            name: format!("host-{id}"),
            identity: Some(format!("install-{id}")),
            remote_farhelm: None,
            remote_state_dir: None,
            state: crate::HostPhase::Connected {
                identity: Some(format!("install-{id}")),
                build_version: "test".to_string(),
                refresh: crate::RefreshHealth::Pending,
            },
            incarnation: 1,
        }
    }

    /// The local option is found by KIND, not by position or by name.
    ///
    /// `ListView` used to rescan the raw registry a second time with a
    /// dedicated `local_host_id` helper to answer this; that helper is
    /// retired and the render site now derives the same id from
    /// `host_options` — the ONE reduction of the registry it already
    /// computes — so this test carries the guarantee `local_host_id`'s own
    /// test used to pin, aimed at `HostOption::local` instead.
    ///
    /// The registry's ordering is the helm's to choose and the local row's
    /// name is prose the helm writes for humans, so either shortcut would
    /// hand the session rows the wrong id — and the row that got it would
    /// then hide the host line for every session on some other machine.
    #[test]
    fn the_local_option_is_identified_by_kind_not_position_or_name() {
        let registry = vec![
            registry_row(4, HostKind::Ssh),
            registry_row(9, HostKind::Local),
            registry_row(11, HostKind::Ssh),
        ];
        let options = host_options(&registry);
        assert_eq!(
            options
                .iter()
                .find(|option| option.local)
                .map(|option| option.id),
            Some(9),
        );
        assert!(
            !options[0].local,
            "the ssh row before the local one stays unmarked"
        );
        assert!(
            !options[2].local,
            "the ssh row after the local one stays unmarked"
        );

        assert_eq!(
            host_options(&[])
                .iter()
                .find(|option| option.local)
                .map(|option| option.id),
            None,
            "no registry in hand names no local row"
        );
        assert_eq!(
            host_options(&[registry_row(4, HostKind::Ssh)])
                .iter()
                .find(|option| option.local)
                .map(|option| option.id),
            None,
            "a fleet of ssh rows has no local row for a session to match"
        );
    }

    /// A session is local only when its host id IS the registry's local
    /// row; every absence answers "not local" and leaves the host line on.
    ///
    /// The absences are the half worth pinning. A helm too old to send
    /// `host`, and a hosts read that has not landed, both leave the UI with
    /// no evidence either way — and the row must name its host rather than
    /// assume the session is on the machine the user is already looking at.
    #[test]
    fn locality_needs_both_ids_and_never_guesses() {
        assert!(session_is_local(Some(9), Some(9)));
        assert!(!session_is_local(Some(4), Some(9)));
        assert!(
            !session_is_local(None, Some(9)),
            "a session naming no host is not evidence of being local"
        );
        assert!(
            !session_is_local(Some(9), None),
            "no registry yet is not evidence of being local either"
        );
    }
}
