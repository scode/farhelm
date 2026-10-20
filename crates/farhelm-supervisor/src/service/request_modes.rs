//! Every control-message variant declares its existing execution and ownership.
//!
//! This is a description of dispatch, not a scheduler. Handlers still decide
//! where to validate, acquire admission, spawn work, send replies and release
//! guards. Moving any of those decisions here would change error precedence or
//! frame ordering. The declaration also constructs the conformance test's
//! representatives, so choosing a mode for a new inbound variant cannot omit
//! that variant from the test's input set.

use farhelm_proto::ControlMsg;

/// The execution and cancellation contract a control-message handler uses.
///
/// These modes describe full-authority connections unless explicitly stated
/// otherwise. Restricted connections refuse ordinary requests before entering
/// their handlers; `RestrictedRelay` documents the one authority-dependent
/// operation. None of the modes promises that a backpressured reply writer can
/// never wait. The conformance contract concerns management admission, lifecycle
/// operations and the other shared waits named by each mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "descriptive contract consumed by conformance tests"
    )
)]
pub(crate) enum ExecutionClass {
    /// Apply a short control operation on the reader, preserving arrival order.
    /// Existing local locks and terminal cleanup remain inline, but this mode
    /// does not wait for management admission, create admission or a lifecycle
    /// operation. Credit, cancellation and attachment controls belong here so
    /// the next frame observes the preceding route operation.
    InlineControl,
    /// Reserve or install a connection-local channel before reading its next
    /// frame. Attach also reserves its reply position before starting output,
    /// so replay cannot overtake its reply. Slow transfer work runs separately;
    /// a contested tab lifecycle claim is refused without waiting. Spawning
    /// the whole handler would race later data, resize, detach or abort frames.
    InlineChannel,
    /// Run the checkout preview's ordinarily brief filesystem and store scan
    /// inline, without taking directory admission. SPEC.md's slow-host policy
    /// accepts this scan delaying that host's terminal I/O; this is distinct
    /// from waiting out a management operation or spawning a long-lived job.
    InlinePreview,
    /// Spawn a connection-owned task without general management admission.
    /// StatFile uses this mode: the reader does not wait for filesystem work,
    /// and connection shutdown can drain or cancel the tracked task.
    ConnectionTask,
    /// Refuse immediately if general admission is full, otherwise run in a
    /// connection-owned tracked handler. Its permit stays through the reply;
    /// shared waits and subprocess work happen after leaving the reader. Core
    /// tab and trash operations may hand accepted work to owned tasks that
    /// outlive this handler and its permit; this class describes the wire
    /// handler's ownership, not a cancellation bound on every backend task.
    AdmittedConnectionTask,
    /// Spawn a connection-owned listing task, then wait on the separate list
    /// budget inside that task. Management saturation cannot prevent listing,
    /// and saturation of the list's own budget cannot hold the reader.
    ListingTask,
    /// Refuse immediately if general admission is full, otherwise hand the
    /// accepted mutation and permit to a supervisor-owned task. The connection
    /// tracks only its reply waiter, so disconnect cannot cancel accepted work
    /// or release admission before its lifecycle operation settles.
    OwnedMutation,
    /// Spawn an owned Delete before acquiring its agent-request fence and
    /// general admission, then take directory admission and the lifecycle
    /// claim there. Waiting deletes neither park the reader nor consume every
    /// management permit merely while an agent request retains its fence.
    /// The owned operation returns its permit to the connection's tracked
    /// reply waiter, which retains it through reply delivery.
    OwnedDelete,
    /// Validate the rename's title, refuse busy admission without waiting,
    /// then move one permit through its owned commit and connection-owned
    /// reply build. The commit survives disconnect; reply construction does
    /// not. A second admission between these phases would change semantics.
    RenameHandoff,
    /// Refuse on full-authority connections; serialize the dedicated restricted
    /// connection's request through cheap shape checks, its mutation fence
    /// when applicable, credential validation under that fence, and the helm
    /// round trip. This mode deliberately does
    /// not claim that the restricted reader keeps processing later requests
    /// while waiting for the helm's answer.
    RestrictedRelay,
    /// Handshake traffic, replies or supervisor events, rather than an ordinary
    /// inbound request. AgentResponse genuinely travels into the supervisor:
    /// the full-authority loop completes it against its own pending upcall
    /// table before dispatch, without management or lifecycle admission.
    /// Hello precedes the loop and a repeated Hello is ignored; unexpected
    /// reverse-direction traffic is logged or refused by the authority gate.
    /// These messages have no request fixture.
    NotRequest,
}

/// Describe a message without performing its admission or execution.
///
/// The macro expands to an exhaustive match with no wildcard. Each inbound
/// entry also requires a representative expression; non-request variants
/// have explicit patterns but no fixture. Keeping the two outputs together
/// forces a new message's author to make both choices in the same declaration.
macro_rules! declare_execution_classes {
    (
        inbound {
            $( $variant:ident => $class:ident => |$context:ident| $message:expr; )*
        }
        not_requests {
            $( $reverse:pat ),+ $(,)?
        }
    ) => {
        /// The existing execution mode for this variant, across the connection
        /// pre-dispatch handling and both authority-selected dispatchers.
        #[cfg_attr(not(test), allow(dead_code, reason = "descriptive contract consumed by conformance tests"))]
        pub(crate) fn execution_class(message: &ControlMsg) -> ExecutionClass {
            match message {
                $(ControlMsg::$variant { .. } => ExecutionClass::$class,)*
                $($reverse => ExecutionClass::NotRequest,)+
            }
        }

        /// Build every declared inbound variant from the caller's owned fixture.
        /// The per-entry assertion catches a constructor accidentally copied
        /// from another variant before it could silently weaken coverage.
        #[cfg(test)]
        pub(crate) fn conformance_requests(context: &ConformanceContext<'_>) -> Vec<ControlMsg> {
            vec![$({
                let $context = context;
                let message = $message;
                assert!(
                    matches!(&message, ControlMsg::$variant { .. }),
                    "the {} representative constructed {}",
                    stringify!($variant),
                    message.variant_name(),
                );
                message
            },)*]
        }
    };
}

/// Inputs belonging to one conformance fixture, never shared global test state.
///
/// Publish `session_id` in memory and in the store, use an existing writable
/// UTF-8 `cwd` and regular `file_path`, and supply a declared launch that passes
/// cheap create validation. Allocating requests need an unused `channel`;
/// channel controls need their matching live route prepared by the caller.
/// It must differ from the progress oracle's channel. A held lifecycle claim
/// lets the tab-attach representative reach its contested-claim refusal even
/// before `tab_id` resolves; tests exercising successful attach must seed it.
/// `relay_port` belongs to the fixture's loopback listener, allocated by the
/// kernel. Constructors do not allocate resources, claim locks or send frames.
#[cfg(test)]
pub(crate) struct ConformanceContext<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) cwd: &'a str,
    pub(crate) file_path: &'a str,
    pub(crate) tab_id: &'a str,
    pub(crate) launch: &'a farhelm_proto::SessionLaunch,
    pub(crate) channel: u32,
    pub(crate) req_id: u64,
    pub(crate) intent_key: &'a str,
    pub(crate) relay_port: std::num::NonZeroU16,
}

declare_execution_classes! {
    inbound {
        BeginTcpRelay => InlineChannel => |c| ControlMsg::BeginTcpRelay {
            req_id: c.req_id,
            session_id: c.session_id.into(),
            channel: c.channel,
            address: Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
            port: c.relay_port.get(),
        };
        TcpRelayAck => InlineControl => |c| ControlMsg::TcpRelayAck {
            channel: c.channel, received: 0,
        };
        CloseTcpRelay => InlineControl => |c| ControlMsg::CloseTcpRelay { channel: c.channel };
        SetBrowserForwarding => InlineControl => |c| ControlMsg::SetBrowserForwarding {
            req_id: c.req_id, enabled: false,
        };
        StatFile => ConnectionTask => |c| ControlMsg::StatFile {
            req_id: c.req_id, session_id: c.session_id.into(), path: c.file_path.into(),
        };
        BeginDownload => InlineChannel => |c| ControlMsg::BeginDownload {
            req_id: c.req_id, session_id: c.session_id.into(),
            path: c.file_path.into(), channel: c.channel,
        };
        DownloadAck => InlineControl => |c| ControlMsg::DownloadAck {
            channel: c.channel, received: 0,
        };
        AbortDownload => InlineControl => |c| ControlMsg::AbortDownload { channel: c.channel };
        CreateSession => OwnedMutation => |c| ControlMsg::CreateSession {
            prompt: None,
            req_id: c.req_id,
            named_start: None,
            parent: None,
            cwd: c.cwd.into(),
            launch: Some(c.launch.clone()),
            inherit_agent: false,
            title: None,
            cols: 80,
            rows: 24,
            intent_key: Some(c.intent_key.into()),
            confirm_yolo: false,
            key_lives_with_session: false,
            request_fingerprint: None,
            github_checkout: None,
        };
        ReconcileGithubCheckout => OwnedMutation => |c| ControlMsg::ReconcileGithubCheckout {
            req_id: c.req_id,
            intent_key: c.intent_key.into(),
            client_identity: "conformance request".into(),
            cols: 80,
            rows: 24,
            refuse_unknown: false,
            inspect_only: false,
            expected_fingerprint: None,
            approve_wrapper_recovery: false,
        };
        ListSessions => ListingTask => |c| ControlMsg::ListSessions { req_id: c.req_id };
        BrowseDirectory => AdmittedConnectionTask => |c| ControlMsg::BrowseDirectory {
            req_id: c.req_id, cwd: c.cwd.into(), membership_only: false,
        };
        ListCheckoutTrash => AdmittedConnectionTask => |c| ControlMsg::ListCheckoutTrash {
            req_id: c.req_id, measure_sizes: false,
        };
        DeleteCheckoutTrash => AdmittedConnectionTask => |c| ControlMsg::DeleteCheckoutTrash {
            req_id: c.req_id, ids: Vec::new(),
        };
        GithubCheckoutPreview => InlinePreview => |c| ControlMsg::GithubCheckoutPreview {
            req_id: c.req_id,
            request: farhelm_proto::GithubPreviewRequest {
                host: None,
                expected_incarnation: None,
                repo: "fixture/project".into(),
                title: None,
                root: Some(c.cwd.into()),
                config_revision: Some(1),
            },
        };
        GithubRepoSearch => AdmittedConnectionTask => |c| ControlMsg::GithubRepoSearch {
            req_id: c.req_id,
            claim: farhelm_proto::ClaimContext { host: "conformance".into(), incarnation: 1 },
            query: "project".into(),
            root: Some(c.cwd.into()),
        };
        StopSession => OwnedMutation => |c| ControlMsg::StopSession {
            req_id: c.req_id, session_id: c.session_id.into(),
        };
        DeleteSession => OwnedDelete => |c| ControlMsg::DeleteSession {
            req_id: c.req_id,
            session_id: c.session_id.into(),
            only_if_nothing_alive: false,
            only_if_agent_ended: false,
            refuse_if_agent_request_in_flight: false,
        };
        RestartSession => OwnedMutation => |c| ControlMsg::RestartSession {
            req_id: c.req_id,
            named_start: None,
            session_id: c.session_id.into(),
            stop_if_running: true,
            with: None,
            expected_launch: None,
        };
        PromptTargetInfo => AdmittedConnectionTask => |c| ControlMsg::PromptTargetInfo {
            req_id: c.req_id, session_id: c.session_id.into(),
        };
        PromptSession => OwnedMutation => |c| ControlMsg::PromptSession {
            req_id: c.req_id, session_id: c.session_id.into(), prompt: "conformance".into(),
        };
        RenameSession => RenameHandoff => |c| ControlMsg::RenameSession {
            req_id: c.req_id, session_id: c.session_id.into(),
            title: "conformance".into(), expected_title: None,
        };
        OpenTab => AdmittedConnectionTask => |c| ControlMsg::OpenTab {
            req_id: c.req_id, session_id: c.session_id.into(),
        };
        CloseTab => AdmittedConnectionTask => |c| ControlMsg::CloseTab {
            req_id: c.req_id, session_id: c.session_id.into(), tab_id: c.tab_id.into(),
        };
        Attach => InlineChannel => |c| ControlMsg::Attach {
            req_id: c.req_id,
            session_id: c.session_id.into(),
            channel: c.channel,
            cols: 80,
            rows: 24,
            terminal: farhelm_proto::TerminalSelector::Tab { id: c.tab_id.into() },
            lease: "conformance".into(),
            if_unowned: false,
        };
        Detach => InlineControl => |c| ControlMsg::Detach { channel: c.channel };
        Resize => InlineControl => |c| ControlMsg::Resize {
            session_id: c.session_id.into(), channel: c.channel, cols: 80, rows: 24,
        };
        PauseOutput => InlineControl => |c| ControlMsg::PauseOutput { channel: c.channel };
        ResumeOutput => InlineControl => |c| ControlMsg::ResumeOutput { channel: c.channel };
        BeginUpload => InlineChannel => |c| ControlMsg::BeginUpload {
            req_id: c.req_id, session_id: c.session_id.into(),
            channel: c.channel, filename: "conformance.txt".into(), size: 0,
        };
        CommitUpload => InlineControl => |c| ControlMsg::CommitUpload {
            req_id: c.req_id, channel: c.channel,
        };
        AbortUpload => InlineControl => |c| ControlMsg::AbortUpload { channel: c.channel };
        AgentRequest => RestrictedRelay => |c| ControlMsg::AgentRequest {
            req_id: c.req_id,
            session_id: c.session_id.into(),
            request: farhelm_proto::AgentVerb::Rename {
                session_id: Some(c.session_id.into()),
                expected_title: Some("conformance".into()),
                title: "conformance renamed".into(),
            },
        };
    }
    not_requests {
        ControlMsg::TcpRelayStarted { .. },
        ControlMsg::TcpRelayEnded { .. },
        ControlMsg::BrowserForwardingSet { .. },
        ControlMsg::FileStat { .. },
        ControlMsg::DownloadStarted { .. },
        ControlMsg::DownloadEnded { .. },
        ControlMsg::Hello { .. },
        ControlMsg::SessionCreated { .. },
        ControlMsg::GithubCheckoutReconciled { .. },
        ControlMsg::SessionList { .. },
        ControlMsg::DirectoryListing { .. },
        ControlMsg::CheckoutTrashListed { .. },
        ControlMsg::CheckoutTrashDeleted { .. },
        ControlMsg::GithubCheckoutPreviewed { .. },
        ControlMsg::GithubRepoResults { .. },
        ControlMsg::SessionStopped { .. },
        ControlMsg::SessionDeleted { .. },
        ControlMsg::SessionRestarted { .. },
        ControlMsg::PromptTarget { .. },
        ControlMsg::SessionPrompted { .. },
        ControlMsg::SessionRenamed { .. },
        ControlMsg::TabOpened { .. },
        ControlMsg::TabClosed { .. },
        ControlMsg::Attached { .. },
        ControlMsg::Detached { .. },
        ControlMsg::ReplayComplete { .. },
        ControlMsg::SessionsChanged,
        ControlMsg::UploadStarted { .. },
        ControlMsg::UploadAck { .. },
        ControlMsg::UploadCommitted { .. },
        ControlMsg::UploadAborted { .. },
        ControlMsg::AgentResponse { .. },
        ControlMsg::Error { .. },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every generated representative must describe one distinct inbound
    /// variant. This exercises construction only: the borrowed path and port
    /// values are never opened, bound or connected. Wire conformance supplies
    /// actual owned resources and checks that each representative reaches its
    /// intended contention boundary.
    #[test]
    fn declaration_constructs_distinct_inbound_messages() {
        let launch = farhelm_proto::SessionLaunch::plain_command("conformance-command");
        let context = ConformanceContext {
            session_id: "conformance-session",
            cwd: "/conformance-cwd",
            file_path: "/conformance-cwd/file",
            tab_id: "conformance-tab",
            launch: &launch,
            channel: 2,
            req_id: 7,
            intent_key: "conformance-intent",
            relay_port: std::num::NonZeroU16::MIN,
        };
        let mut seen = std::collections::HashSet::new();
        for message in conformance_requests(&context) {
            assert!(
                seen.insert(message.variant_name()),
                "duplicate representative for {}",
                message.variant_name(),
            );
            assert_ne!(
                execution_class(&message),
                ExecutionClass::NotRequest,
                "{} must describe real inbound handling",
                message.variant_name(),
            );
        }
    }
}
