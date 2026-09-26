//! Fresh-checkout destination and retry facts that must survive async UI work.
//!
//! The helm and supervisor validate again before allocation. These types keep
//! the browser's displayed preview, installation claim and submitted request
//! together; a late response is not permission to change the user's destination.

use farhelm_proto::RepoError;
use serde::{Deserialize, Serialize};

/// The launch destination is independent of the agent controls and search
/// text. Existing retains the exact submitted path; the separate text editor
/// may show an escaped spelling of that path. A GitHub destination never uses
/// that dormant editor value as a fallback while its preview is unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DestinationDraft {
    Existing {
        cwd: String,
    },
    Github {
        repo: GithubRepo,
        // Ready carries the complete authority and reply. Keep that larger
        // snapshot indirect so ordinary destination drafts stay compact.
        preview_state: Box<PreviewState>,
    },
}

/// A preview is usable only with its request's complete authority. Keeping
/// that authority with both success and failure prevents late completions
/// from replacing the state of a newer draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreviewState {
    Pending,
    Ready {
        authority: PreviewAuthority,
        preview: GithubPreview,
    },
    Failed {
        authority: PreviewAuthority,
        message: String,
    },
}

impl DestinationDraft {
    /// Selecting a repo is explicit fresh intent, even if it is the same repo
    /// as the prior selection. Never inherit that selection's preview.
    pub(crate) fn github(repo: GithubRepo) -> Self {
        Self::Github {
            repo,
            preview_state: Box::new(PreviewState::Pending),
        }
    }

    /// Return the repo intent without exposing an old checkout path.
    pub(crate) fn repo(&self) -> Option<&GithubRepo> {
        match self {
            Self::Existing { .. } => None,
            Self::Github { repo, .. } => Some(repo),
        }
    }
}

/// Repository intent, independent of any previous checkout's actual cwd.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GithubRepo {
    pub(crate) owner: String,
    pub(crate) name: String,
}

/// Read-only association reported by the supervisor registry. The origin id
/// records who allocated the checkout, not who must remain alive to retain it.
/// Neither this association nor repo provenance changes an ordinary Clone's
/// destination: that action still uses the source session's actual cwd.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct WorkingCopyInfo {
    pub(crate) id: String,
    pub(crate) repo: GithubRepo,
    pub(crate) canonical_path: String,
    pub(crate) origin_session_id: String,
}

impl GithubRepo {
    /// Validate the composer's complete owner/repo pair without repairing URLs
    /// or stripping transport suffixes.
    ///
    /// The rules are `farhelm_proto::parse_github_repo`'s, the same function
    /// the helm and the supervisor run, rather than a copy kept here: a copy
    /// that fell behind would offer repositories the server then refuses, or
    /// hide ones it would accept. Server validation remains authoritative;
    /// this only decides what the composer offers.
    ///
    /// The struct itself is still this crate's own rather than proto's
    /// `GithubRepo`, which has the same two fields: it is threaded through the
    /// UI's API and form code with its own `identifier()`, and unifying the
    /// types is a separate change from sharing the rules.
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match farhelm_proto::parse_github_repo(value) {
            Ok(repo) => Ok(Self {
                owner: repo.owner,
                name: repo.name,
            }),
            Err(RepoError::Empty | RepoError::MissingSeparator) => {
                Err("enter a GitHub repository as owner/repo".into())
            }
            Err(
                RepoError::ExtraSeparator
                | RepoError::InvalidOwner
                | RepoError::InvalidRepo
                | RepoError::TooLong,
            ) => Err("enter a valid GitHub owner/repo pair without a URL or branch suffix".into()),
        }
    }

    /// Canonical identifier used for search, grouping and the create body.
    pub(crate) fn identifier(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Exact displayed destination, including the installation that proposed it.
/// The flattened shape mirrors the helm's AcceptedGithubPreview response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GithubPreview {
    pub(crate) canonical_root: String,
    pub(crate) basename: String,
    pub(crate) cwd: String,
    pub(crate) config_revision: i64,
    pub(crate) host: String,
    pub(crate) incarnation: u64,
    pub(crate) installation_identity: String,
}

/// A create carries the same repo, title and preview the user accepted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GithubCheckoutRequest {
    pub(crate) repo: String,
    pub(crate) title: Option<String>,
    pub(crate) preview: GithubPreview,
}

/// Completion can remain useful even when the target cannot finish its scan.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct GithubRepositories {
    pub(crate) host: String,
    pub(crate) incarnation: u64,
    pub(crate) installation_identity: String,
    pub(crate) repos: Vec<GithubRepo>,
    pub(crate) truncated: bool,
    pub(crate) scan_error: Option<String>,
}

/// Discovery is tied to both its query and the destination generation. A
/// completed request from before a repo selection cannot repopulate search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositoryAuthority {
    pub(crate) generation: u64,
    pub(crate) host: i64,
    pub(crate) incarnation: u64,
    pub(crate) installation_identity: String,
    pub(crate) destination_generation: u64,
    pub(crate) query: String,
}

impl RepositoryAuthority {
    /// The helm echoes installation identity independently of registry row id.
    pub(crate) fn accepts(&self, live: &Self, reply: &GithubRepositories) -> bool {
        self == live
            && reply.host == self.host.to_string()
            && reply.incarnation == self.incarnation
            && !self.installation_identity.is_empty()
            && reply.installation_identity == self.installation_identity
    }
}

/// A valid manually typed pair remains actionable even if discovery failed
/// or reached its cap. Preserve server ordering and offer the manual pair once.
pub(crate) fn repository_choices(query: &str, discovered: &[GithubRepo]) -> Vec<GithubRepo> {
    let query = query.trim().to_ascii_lowercase();
    let mut choices = Vec::new();
    if let Ok(repo) = GithubRepo::parse(&query) {
        choices.push(repo);
    }
    for repo in discovered {
        if choices.len() == 100 {
            break;
        }
        if repo.identifier().contains(&query) && !choices.contains(repo) {
            choices.push(repo.clone());
        }
    }
    choices
}

/// Request-generation identity for a proposed destination. Agent selection is
/// included because switching launch modes also invalidates the displayed offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreviewAuthority {
    pub(crate) generation: u64,
    /// Latest configuration epoch observed through the authenticated history
    /// read. A preview may see a newer commit, but never an older one.
    pub(crate) config_revision: i64,
    pub(crate) host: String,
    pub(crate) incarnation: u64,
    pub(crate) installation_identity: String,
    pub(crate) repo: GithubRepo,
    pub(crate) title: Option<String>,
    pub(crate) agent: String,
}

impl PreviewAuthority {
    /// A completion must still describe both the current draft and the same
    /// installation. Matching a host row alone cannot survive adoption safely.
    pub(crate) fn accepts(&self, live: &Self, preview: &GithubPreview) -> bool {
        self == live
            && preview.config_revision >= self.config_revision
            && preview.host == self.host
            && preview.incarnation == self.incarnation
            && !self.installation_identity.is_empty()
            && preview.installation_identity == self.installation_identity
    }
}

/// Fresh creates need more than an HTTP status to decide whether to retire a
/// key. Unknown outcomes include retained refusals and errors after allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FreshCreateError {
    /// The authenticated server proved this exact request's key is permanently
    /// refused on its original installation, with no accepted allocation.
    /// This resolves even an earlier lost response; a generic 409 cannot.
    Unaccepted(String),
    /// No such proof: a transport failure or an unresolved/previously accepted key.
    Unresolved(String),
}

impl FreshCreateError {
    /// Preserve the actionable server text independently of outcome policy.
    pub(crate) fn message(&self) -> &str {
        match self {
            Self::Unaccepted(text) | Self::Unresolved(text) => text,
        }
    }
}

/// The exact dispatched payload and key outlive preview/config refreshes.
/// Ambiguous outcomes retain that request until success or durable proof of
/// refusal resolves it. Configuration changes and ordinary conflicts cannot
/// establish that proof; only reconciliation of this key on its original
/// installation can make another explicit submission safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubAttempt {
    pub(crate) key: String,
    pub(crate) body: serde_json::Value,
    pub(crate) installation_identity: String,
}

impl GithubAttempt {
    /// Capture before dispatch; callers must recheck the draft after key minting.
    pub(crate) fn new(key: String, body: serde_json::Value, installation_identity: String) -> Self {
        Self {
            key,
            body,
            installation_identity,
        }
    }

    /// Durable refusal settles the key itself, including earlier lost replies.
    /// The caller may refresh the preview, but must wait for another explicit
    /// submission. Unresolved errors never authorize replacing this request.
    pub(crate) fn may_retire_after(&self, error: &FreshCreateError) -> bool {
        matches!(error, FreshCreateError::Unaccepted(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Discovery must not turn scan failure or truncation into a ban on an
    /// explicitly named repo. Suggestions remain unique and bounded.
    #[test]
    fn manual_repository_choice_survives_empty_and_capped_discovery() {
        let manual = GithubRepo::parse("acme/bar").unwrap();
        assert_eq!(repository_choices("Acme/Bar", &[]), vec![manual.clone()]);
        assert!(repository_choices("acme/bar@main", &[]).is_empty());
        let discovered = (0..120)
            .map(|n| GithubRepo::parse(&format!("acme/bar-{n}")).unwrap())
            .collect::<Vec<_>>();
        let choices = repository_choices("acme/bar", &discovered);
        assert_eq!(choices.len(), 100);
        assert_eq!(choices[0], manual);
        assert_eq!(
            repository_choices("acme/bar", &[manual.clone(), manual.clone()]),
            vec![manual]
        );
    }

    /// Returning to the same query or host after an intervening edit does not
    /// restore an old response's authority, even when its visible text matches.
    #[test]
    fn repository_reply_requires_query_generation_and_installation() {
        let authority = RepositoryAuthority {
            generation: 1,
            host: 1,
            incarnation: 2,
            installation_identity: "install-a".into(),
            destination_generation: 3,
            query: "acme".into(),
        };
        let reply = GithubRepositories {
            host: "1".into(),
            incarnation: 2,
            installation_identity: "install-a".into(),
            repos: vec![GithubRepo::parse("acme/bar").unwrap()],
            truncated: true,
            scan_error: Some("scan unavailable".into()),
        };
        assert!(authority.accepts(&authority, &reply));
        let mut live = authority.clone();
        live.generation += 2;
        assert!(!authority.accepts(&live, &reply));
        let mut live = authority.clone();
        live.destination_generation += 1;
        assert!(!authority.accepts(&live, &reply));
        let mut changed_install = reply.clone();
        changed_install.installation_identity = "install-b".into();
        assert!(!authority.accepts(&authority, &changed_install));
    }

    /// Inputs accepted locally must remain plain repository identities. URL
    /// repair and suffix stripping would silently select a different repository.
    /// The composer's two refusal sentences also map onto proto's failure
    /// classes: a missing pair asks for one, anything else says why not.
    #[test]
    fn repo_input_preserves_identity_and_refuses_transport_syntax() {
        assert_eq!(
            GithubRepo::parse("Acme/Bar.git").unwrap().identifier(),
            "acme/bar.git"
        );
        for text in [
            "",
            "bar",
            "acme/bar/extra",
            "https://github.com/acme/bar",
            "acme/bar@main",
            "acme/..",
            "a--b/bar",
            "acme/bar\n",
            "acme/%62ar",
            "-acme/bar",
            "acme /bar",
        ] {
            assert!(GithubRepo::parse(text).is_err(), "accepted {text:?}");
        }
        assert!(GithubRepo::parse(&format!("{}/{}", "a".repeat(39), "b".repeat(100))).is_ok());
        assert!(GithubRepo::parse(&format!("{}/bar", "a".repeat(40))).is_err());
        assert!(GithubRepo::parse(&format!("acme/{}", "b".repeat(101))).is_err());
        // The composer's two sentences survive delegating to proto's parser:
        // a missing pair asks for one, anything else malformed says why not.
        assert_eq!(
            GithubRepo::parse("bar").unwrap_err(),
            "enter a GitHub repository as owner/repo"
        );
        assert_eq!(
            GithubRepo::parse("acme/bar/extra").unwrap_err(),
            "enter a valid GitHub owner/repo pair without a URL or branch suffix"
        );
    }

    /// Generation, installation and draft identity are separate boundaries.
    /// An unchanged registry id cannot authorize a reply after a draft edit or
    /// adoption, even when a restarted helm reused its connection counter.
    #[test]
    fn preview_response_requires_the_whole_live_authority() {
        let authority = PreviewAuthority {
            generation: 1,
            config_revision: 1,
            host: "1".into(),
            incarnation: 2,
            installation_identity: "installation-a".into(),
            repo: GithubRepo::parse("acme/bar").unwrap(),
            title: None,
            agent: "codex".into(),
        };
        let preview = GithubPreview {
            canonical_root: "/work".into(),
            basename: "bar-1".into(),
            cwd: "/work/bar-1".into(),
            config_revision: 1,
            host: "1".into(),
            incarnation: 2,
            installation_identity: "installation-a".into(),
        };
        assert!(authority.accepts(&authority, &preview));
        assert!(
            !authority.accepts(
                &authority,
                &GithubPreview {
                    config_revision: 0,
                    ..preview.clone()
                }
            ),
            "a response older than observed configuration must never enable Launch"
        );
        assert!(
            authority.accepts(
                &authority,
                &GithubPreview {
                    config_revision: 2,
                    ..preview.clone()
                }
            ),
            "preview may observe a committed change before the feed-driven history read"
        );
        for changed in [
            PreviewAuthority {
                config_revision: 2,
                ..authority.clone()
            },
            PreviewAuthority {
                generation: 2,
                ..authority.clone()
            },
            PreviewAuthority {
                installation_identity: "installation-b".into(),
                ..authority.clone()
            },
            PreviewAuthority {
                title: Some("fix".into()),
                ..authority.clone()
            },
            PreviewAuthority {
                agent: "raw".into(),
                ..authority.clone()
            },
            PreviewAuthority {
                repo: GithubRepo::parse("acme/other").unwrap(),
                ..authority.clone()
            },
        ] {
            assert!(!authority.accepts(&changed, &preview));
        }
        assert!(!authority.accepts(
            &authority,
            &GithubPreview {
                incarnation: 3,
                ..preview.clone()
            }
        ));
        assert!(!authority.accepts(
            &authority,
            &GithubPreview {
                installation_identity: "installation-b".into(),
                ..preview
            }
        ));
    }

    /// A lost reply and generic conflict retain the original request, while
    /// a permanent refusal resolves uncertainty about every dispatch of its
    /// key. Neither classification rewrites the captured payload or identity.
    #[test]
    fn ambiguous_attempt_retires_only_after_durable_refusal() {
        let body =
            serde_json::json!({"intent_key":"original", "github_checkout":{"repo":"acme/bar"}});
        let attempt = GithubAttempt::new("original".into(), body.clone(), "installation-a".into());
        assert!(!attempt.may_retire_after(&FreshCreateError::Unresolved("reply lost".into())));
        assert!(
            !attempt.may_retire_after(&FreshCreateError::Unresolved("generic conflict".into()))
        );
        assert!(
            attempt.may_retire_after(&FreshCreateError::Unaccepted("permanently refused".into()))
        );
        assert_eq!(attempt.key, "original");
        assert_eq!(attempt.body, body);
        assert_eq!(attempt.installation_identity, "installation-a");
        let never_accepted = GithubAttempt::new("new".into(), body, "installation-a".into());
        assert!(
            never_accepted
                .may_retire_after(&FreshCreateError::Unaccepted("preview changed".into()))
        );
    }
}
