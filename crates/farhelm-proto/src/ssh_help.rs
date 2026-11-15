//! The reply of the helm's `POST /api/ssh-connections`: ready-to-paste commands that start the helm's two shared SSH
//! connections to a host by hand, for hosts that ask for a second factor on every login (SPEC.md, "Feedback": "SSH
//! needs 2FA?").
//!
//! The helm composes every command itself, from its own state directory, connection overrides and socket fit checks,
//! so a client shows and copies them and never builds one. The types live here so the helm serializes and the GUI
//! decodes the same ones (SPEC_impl.md, "The UI channel": no hand-written mirrors on either side). The request is a
//! read with a body (an optional destination typed in Add host) and is not typed here: the helm reads it as JSON with
//! one optional `destination` field.
//!
//! Every string in here is the helm's or a host's, never the client's own words: a registered host's name and
//! destination, a command quoting a destination and a path, and the reason a connection's command cannot be composed,
//! which quotes the helm's state path. A GUI holds them as host text.

use serde::{Deserialize, Serialize};

/// One destination's two commands. A command is `None` when the helm cannot compose it for that connection; the reason
/// is in [`Help::unavailable`], once for every host, since it is the helm's state path that fails, not the host.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HostCommands {
    /// The registered host's display name (its alias, else its destination), or the helm's own heading for the
    /// destination typed in Add host ("The host you're adding").
    pub name: String,
    /// The SSH destination the commands connect to, as registered or as typed.
    pub destination: String,
    /// The command that starts the Sessions connection (the one the helm's supervisor link uses).
    pub sessions: Option<String>,
    /// The command that starts the Setup and updates connection (the one provisioning uses).
    pub setup: Option<String>,
}

/// The whole reply: what cannot be composed, then the commands of every registered SSH host and of the typed
/// destination, if one was sent.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Help {
    /// One line per connection whose command cannot be composed (the state path too long for its socket, holding a
    /// literal `${`, or not UTF-8), naming the connection and the reason. Empty when both can be.
    pub unavailable: Vec<String>,
    /// Every registered SSH host, in the registry's order. Other kinds of host (the local host, sandboxes,
    /// containers) are not listed.
    pub hosts: Vec<HostCommands>,
    /// The commands for the destination the request named, or `None` when it named none or an empty one.
    pub extra: Option<HostCommands>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The GUI decodes what the helm serializes, so the field names and the explicit `null`s are the contract both
    /// sides keep. Specified: a reply with an unavailable line, one host missing its setup command and no extra
    /// serializes to exactly this JSON, and that JSON decodes back to the same value.
    #[test]
    fn the_reply_keeps_its_wire_shape() {
        let help = Help {
            unavailable: vec!["setup and updates connection: path too long".into()],
            hosts: vec![HostCommands {
                name: "build-box".into(),
                destination: "dev@buildbox".into(),
                sessions: Some("ssh -N -f -- dev@buildbox".into()),
                setup: None,
            }],
            extra: None,
        };
        let wire = serde_json::json!({
            "unavailable": ["setup and updates connection: path too long"],
            "hosts": [{
                "name": "build-box",
                "destination": "dev@buildbox",
                "sessions": "ssh -N -f -- dev@buildbox",
                "setup": null,
            }],
            "extra": null,
        });
        assert_eq!(serde_json::to_value(&help).unwrap(), wire);
        assert_eq!(serde_json::from_value::<Help>(wire).unwrap(), help);
    }
}
