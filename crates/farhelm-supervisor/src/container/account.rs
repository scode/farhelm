//! Image account selection happens before the checkout mount can manufacture a
//! plausible home directory. Only the creation ticket carries the host identity;
//! the resulting container configuration supplies identity and HOME to every exec.

use super::{ContainerRuntime, control_command};
use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The supervisor's account, obtained from the host user database rather than
/// shell environment. Tests can inject a different image-selection premise without
/// changing the test runner's environment or requiring a specially built image.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostAccount {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
}

impl HostAccount {
    /// Resolve the supervisor's effective uid through NSS. A missing or unusable
    /// record refuses creation: guessing USER/HOME would select another account.
    pub(crate) fn current() -> anyhow::Result<Self> {
        let uid = unsafe { libc::geteuid() };
        let mut size = 16384;
        loop {
            let mut buffer = vec![0u8; size];
            // SAFETY: getpwuid_r writes this record and buffer, and returned
            // pointers are copied before either allocation leaves this iteration.
            let mut record: libc::passwd = unsafe { std::mem::zeroed() };
            let mut result = std::ptr::null_mut();
            let code = unsafe {
                libc::getpwuid_r(
                    uid,
                    &mut record,
                    buffer.as_mut_ptr().cast(),
                    size,
                    &mut result,
                )
            };
            if code == libc::ERANGE && size < 1024 * 1024 {
                size *= 2;
                continue;
            }
            anyhow::ensure!(
                code == 0,
                "could not read the host account: {}",
                std::io::Error::from_raw_os_error(code)
            );
            anyhow::ensure!(
                !result.is_null(),
                "the host uid has no user database account"
            );
            anyhow::ensure!(
                !record.pw_name.is_null() && !record.pw_dir.is_null(),
                "the host account has no name or home"
            );
            // SAFETY: successful getpwuid_r supplies terminated strings in buffer.
            let name = unsafe { std::ffi::CStr::from_ptr(record.pw_name) }
                .to_str()?
                .to_owned();
            let home = unsafe { std::ffi::CStr::from_ptr(record.pw_dir) }
                .to_str()?
                .into();
            let account = Self {
                name,
                uid: record.pw_uid,
                gid: record.pw_gid,
                home,
            };
            account.validate()?;
            return Ok(account);
        }
    }

    /// Paths become command arguments, never shell source; reject malformed user
    /// database records before publishing a ticket rather than silently normalizing.
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.name.is_empty() && !self.name.contains([':', '\n', '\0']),
            "the host account has an unusable name"
        );
        anyhow::ensure!(
            self.home.is_absolute()
                && self.home != Path::new("/")
                && self
                    .home
                    .to_str()
                    .is_some_and(|home| !home.contains(['\n', '\0'])),
            "the host account has an unusable home path"
        );
        Ok(())
    }
}

/// A selected image account is creation-local. `create` authorizes Ubuntu's
/// account tools; `create_group` is needed only when the requested gid is free.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Choice {
    pub account: HostAccount,
    create: bool,
    create_group: bool,
    /// Decided before mounting: Podman can manufacture a missing account home
    /// as a root-owned checkout parent, which still needs ownership and skel.
    prepare_home: bool,
}

/// A passwd record and the existence of its home in the unmounted image must
/// stay together; uid aliases may name different home paths.
#[derive(Debug, PartialEq, Eq)]
struct ImageUser {
    account: HostAccount,
    home_exists: bool,
}

/// Facts from the unmounted image, not from the keeper's bind-created parents.
struct ImageAccounts {
    users: Vec<ImageUser>,
    groups: Vec<(String, u32)>,
    home_owner: Option<u32>,
}

impl ImageAccounts {
    /// NUL framing keeps file contents separate without reserving a legal passwd
    /// name or pathname as a sentinel. Malformed or missing facts fail closed.
    fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        let fields: Vec<_> = bytes.split(|byte| *byte == 0).collect();
        anyhow::ensure!(
            fields.len() == 4,
            "image account inspection returned malformed facts"
        );
        let mut users = Vec::new();
        for line in std::str::from_utf8(fields[0])?
            .lines()
            .filter(|line| !line.is_empty())
        {
            let parts: Vec<_> = line.split(':').collect();
            anyhow::ensure!(parts.len() == 7, "image passwd database is malformed");
            users.push(HostAccount {
                name: parts[0].into(),
                uid: parts[2].parse().context("image passwd uid is invalid")?,
                gid: parts[3].parse().context("image passwd gid is invalid")?,
                home: parts[5].into(),
            });
        }
        let mut groups = Vec::new();
        for line in std::str::from_utf8(fields[1])?
            .lines()
            .filter(|line| !line.is_empty())
        {
            let parts: Vec<_> = line.split(':').collect();
            anyhow::ensure!(parts.len() == 4, "image group database is malformed");
            groups.push((
                parts[0].into(),
                parts[2].parse().context("image group gid is invalid")?,
            ));
        }
        let owner = std::str::from_utf8(fields[2])?.trim();
        let home_owner = if owner == "missing" {
            None
        } else {
            Some(owner.parse().context("image home owner is invalid")?)
        };
        let homes: Vec<_> = std::str::from_utf8(fields[3])?
            .lines()
            .map(|line| match line {
                "yes" => Ok(true),
                "no" => Ok(false),
                _ => bail!("image account home existence is invalid"),
            })
            .collect::<anyhow::Result<_>>()?;
        anyhow::ensure!(
            users.len() == homes.len(),
            "image account home facts are incomplete"
        );
        let users = users
            .into_iter()
            .zip(homes)
            .map(|(account, home_exists)| ImageUser {
                account,
                home_exists,
            })
            .collect();
        Ok(Self {
            users,
            groups,
            home_owner,
        })
    }

    /// Preserve the maintainer's rule order, including rule 1 before a preexisting
    /// home. Root-owned and unresolvable homes cannot select an account, so they
    /// fall through to the host uid; a name-only collision has no usable fallback.
    fn choose(&self, host: &HostAccount) -> anyhow::Result<Choice> {
        host.validate()?;
        let by_uid = self.users.iter().find(|user| user.account.uid == host.uid);
        if by_uid.is_none() && !self.users.iter().any(|user| user.account.name == host.name) {
            let create_group = !self.groups.iter().any(|(_, gid)| *gid == host.gid);
            anyhow::ensure!(
                !create_group || !self.groups.iter().any(|(name, _)| name == &host.name),
                "cannot create the host account: its group name already has another gid"
            );
            return Ok(Choice {
                account: host.clone(),
                create: true,
                create_group,
                prepare_home: true,
            });
        }
        if let Some(owner) = self.home_owner.filter(|owner| *owner != 0)
            && let Some(user) = self.users.iter().find(|user| user.account.uid == owner)
        {
            return Ok(Choice {
                account: HostAccount {
                    home: host.home.clone(),
                    ..user.account.clone()
                },
                create: false,
                create_group: false,
                prepare_home: false,
            });
        }
        if let Some(user) = by_uid {
            user.account
                .validate()
                .context("the selected image account is unusable")?;
            return Ok(Choice {
                account: user.account.clone(),
                create: false,
                create_group: false,
                prepare_home: !user.home_exists,
            });
        }
        bail!(
            "cannot choose a container account: the host name exists, but neither its home owner nor its uid selects an account"
        )
    }
}

/// Bash is already required by the keeper. Positional arguments keep host paths
/// out of shell source, and no mount or workdir is supplied to this inspection.
const INSPECT: &str = r#"set -eu
cat /etc/passwd
printf '\0'
cat /etc/group
printf '\0'
if [ -d "$1" ]; then stat -Lc '%u' -- "$1"; else printf 'missing\n'; fi
printf '\0'
# Preserve passwd order, including a final line without a newline. Rust joins
# each existence fact to that exact record, rather than assuming unique uids.
while IFS= read -r line || [ -n "$line" ]; do
    [ -n "$line" ] || continue
    IFS=: read -r name password uid gid gecos home shell <<< "$line"
    if [ -d "$home" ]; then printf 'yes\n'; else printf 'no\n'; fi
done < /etc/passwd
"#;

/// Ubuntu tools establish the account, while a private staging tree receives the
/// recursive chown. Merging skips existing files and the checkout subtree; neither
/// checkout entries nor preexisting home entries are recursively re-owned.
const INITIALIZE: &str = r#"set -eu
name=$1 uid=$2 gid=$3 home=$4 checkout=$5 create=$6 create_group=$7 prepare_home=$8
if [ "$create_group" = yes ]; then groupadd --gid "$gid" -- "$name"; fi
if [ "$create" = yes ]; then
    useradd -M -l --no-user-group --uid "$uid" --gid "$gid" --home-dir "$home" --shell /bin/bash -- "$name"
fi
# This is an image fact, not a mount-time directory check: an existing uid's
# missing home may now exist solely because Podman made the bind's parents.
[ "$prepare_home" = yes ] || exit 0
# Resolve existing symlink parents and .. before the first filesystem edit. Podman
# has already mounted the checkout; a lexical prefix check would miss aliases into it.
checkout=$(realpath -e -- "$checkout")
home=$(realpath -m -- "$home")
if [ "$home" = / ]; then
    printf 'refusing to seed the filesystem root as a container home\n' >&2
    exit 1
fi
case "$home/" in "$checkout/"*)
    if [ ! -d "$home" ]; then
        printf 'container home is inside the checkout but does not exist; refusing to create it there\n' >&2
        exit 1
    fi
    exit 0;;
esac
mkdir -p -- "$home"
chown -- "$uid:$gid" "$home"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
cp -a -- /etc/skel/. "$stage/"
chown -hR -- "$uid:$gid" "$stage"
shopt -s dotglob nullglob
seed() {
    local source target entry destination
    source=$1 target=$2
    for entry in "$source"/*; do
        destination=$target/${entry##*/}
        case "$destination/" in "$checkout/"*) continue;; esac
        if [ -e "$destination" ] || [ -L "$destination" ]; then
            if [ -d "$entry" ] && [ ! -L "$entry" ] && [ -d "$destination" ] && [ ! -L "$destination" ]; then
                seed "$entry" "$destination"
            fi
        else
            cp -a -- "$entry" "$destination"
        fi
    done
}
seed "$stage" "$home"
"#;

impl ContainerRuntime {
    /// Resolve a mutable local name once for this creation. Inspection and keeper
    /// must use the same content even if another process retags the name between
    /// them; the ID stays local to this helper and never becomes launch state.
    pub(super) fn image_id(&self) -> anyhow::Result<String> {
        let output = control_command(&self.podman)
            .args([
                "image",
                "inspect",
                "--format",
                "{{.Id}}",
                &self.config.image,
            ])
            .output()
            .context("could not resolve the local container image")?;
        anyhow::ensure!(
            output.status.success(),
            "could not resolve the local container image: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let id = std::str::from_utf8(&output.stdout)?.trim();
        let digest = id.strip_prefix("sha256:").unwrap_or(id);
        anyhow::ensure!(
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Podman returned an invalid local image ID"
        );
        Ok(id.to_owned())
    }

    /// Resolve the local image's account facts under the creator fence. The
    /// inspection cannot see bind-created parents and cannot reach the network.
    pub(super) fn choose_account(&self, host: &HostAccount, image: &str) -> anyhow::Result<Choice> {
        host.validate()?;
        let output = control_command(&self.podman)
            .args([
                "run",
                "--rm",
                "--pull=never",
                "--network",
                "none",
                "--user",
                "0:0",
                "--entrypoint",
                "bash",
                image,
                "-c",
                INSPECT,
                "--",
            ])
            .arg(&host.home)
            .output()
            .context("could not inspect the image's accounts")?;
        anyhow::ensure!(
            output.status.success(),
            "could not inspect the image's accounts: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        ImageAccounts::parse(&output.stdout)?.choose(host)
    }

    /// Run setup as container root only during creation. The keeper's configured
    /// account remains the default for all later execs, including terminal tabs.
    pub(super) fn initialize_account(
        &self,
        choice: &Choice,
        checkout: &Path,
    ) -> anyhow::Result<()> {
        let output = control_command(&self.podman)
            .args([
                "exec",
                "--user",
                "0:0",
                &self.name()?,
                "bash",
                "-c",
                INITIALIZE,
                "--",
                &choice.account.name,
                &choice.account.uid.to_string(),
                &choice.account.gid.to_string(),
            ])
            .arg(&choice.account.home)
            .arg(checkout)
            .args([
                if choice.create { "yes" } else { "no" },
                if choice.create_group { "yes" } else { "no" },
                if choice.prepare_home { "yes" } else { "no" },
            ])
            .output()
            .context("could not initialize the container account")?;
        anyhow::ensure!(
            output.status.success(),
            "could not initialize the container account: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic user databases keep rule precedence independent of the machine
    /// running the tests, including hosts whose real uid happens to be 1000.
    fn host() -> HostAccount {
        HostAccount {
            name: "fixture".into(),
            uid: 2000,
            gid: 3000,
            home: "/home/fixture".into(),
        }
    }

    /// The stock image premise includes an account whose uid can match the host
    /// independently of its name, which distinguishes fallback from creation.
    fn image() -> ImageAccounts {
        ImageAccounts {
            users: vec![ImageUser {
                account: HostAccount {
                    name: "ubuntu".into(),
                    uid: 1000,
                    gid: 1000,
                    home: "/home/ubuntu".into(),
                },
                home_exists: true,
            }],
            groups: vec![("ubuntu".into(), 1000)],
            home_owner: None,
        }
    }

    /// Absence of both name and uid wins even when an image home already exists;
    /// an existing gid is reused instead of creating a conflicting second group.
    #[test]
    fn absent_name_and_uid_create_first_and_reuse_existing_gid() {
        let mut facts = image();
        facts.home_owner = Some(1000);
        let chosen = facts.choose(&host()).unwrap();
        assert_eq!(chosen.account, host());
        assert!(chosen.create && chosen.create_group && chosen.prepare_home);
        facts.groups.push(("shared".into(), 3000));
        assert!(!facts.choose(&host()).unwrap().create_group);
    }

    /// A home owner takes precedence over uid matching and need not have the host
    /// name or a matching passwd home. HOME remains the explicitly selected path.
    #[test]
    fn preseeded_home_owner_precedes_uid_fallback() {
        let mut facts = image();
        let mut host = host();
        host.uid = 1000;
        facts.users.push(ImageUser {
            account: HostAccount {
                name: "prepared".into(),
                uid: 4000,
                gid: 5000,
                home: "/elsewhere".into(),
            },
            home_exists: false,
        });
        facts.home_owner = Some(4000);
        let chosen = facts.choose(&host).unwrap();
        assert_eq!(chosen.account.name, "prepared");
        assert_eq!(chosen.account.uid, 4000);
        assert_eq!(chosen.account.gid, 5000);
        assert_eq!(chosen.account.home, host.home);
        assert!(!chosen.create && !chosen.prepare_home);
    }

    /// Missing, root-owned and unknown-owner homes cannot choose an account and
    /// use the image account at the host uid, including its own home path.
    #[test]
    fn unusable_home_owners_fall_back_to_existing_uid() {
        let mut facts = image();
        let mut host = host();
        host.uid = 1000;
        for owner in [None, Some(0), Some(9876)] {
            facts.home_owner = owner;
            let chosen = facts.choose(&host).unwrap();
            assert_eq!(chosen.account, facts.users[0].account);
            assert!(!chosen.create && !chosen.prepare_home);
        }
    }

    /// An existing uid whose home is absent in the image still needs preparation
    /// even if mounting a checkout will later manufacture that home as a parent.
    #[test]
    fn missing_uid_home_retains_pre_mount_preparation_decision() {
        let mut facts = image();
        let mut host = host();
        host.uid = 1000;
        facts.users[0].home_exists = false;
        let choice = facts.choose(&host).unwrap();
        assert_eq!(choice.account, facts.users[0].account);
        assert!(!choice.create && choice.prepare_home);
        facts.users[0].home_exists = true;
        assert!(!facts.choose(&host).unwrap().prepare_home);
    }

    /// Podman's bind parents can already exist by setup time. Exercise the real
    /// setup script against that state, redirecting only its skel source through
    /// a child-local cp shim. The chown recorder distinguishes ownership repair
    /// from an accidental early return even though this unprivileged fixture is
    /// already owned by the test uid. Real namespace ownership is checked by e2e.
    #[cfg(target_os = "linux")]
    #[test]
    fn existing_uid_seeds_bind_created_home_without_touching_checkout() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let checkout = home.join("checkouts/repository");
        let sentinel = checkout.join("sentinel");
        let skel = root.path().join("skel");
        let bin = root.path().join("bin");
        let calls = root.path().join("chown-calls");
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::write(&sentinel, b"owned checkout content").unwrap();
        std::fs::set_permissions(&checkout, std::fs::Permissions::from_mode(0o751)).unwrap();
        std::fs::set_permissions(&sentinel, std::fs::Permissions::from_mode(0o640)).unwrap();
        std::fs::create_dir_all(skel.join("checkouts/repository")).unwrap();
        std::fs::write(skel.join(".seeded"), b"skeleton configuration").unwrap();
        std::fs::write(
            skel.join("checkouts/repository/forbidden"),
            b"must not enter checkout",
        )
        .unwrap();
        std::fs::create_dir(&bin).unwrap();
        let cp = crate::scope::resolve_program("cp").unwrap();
        let chown = crate::scope::resolve_program("chown").unwrap();
        let quote = |path: &Path| shell_words::quote(path.to_str().unwrap()).into_owned();
        std::fs::write(bin.join("cp"), format!("#!/bin/sh\nif [ \"$3\" = /etc/skel/. ]; then exec {} \"$1\" \"$2\" {} \"$4\"; else exec {} \"$@\"; fi\n", quote(&cp), quote(&skel.join(".")), quote(&cp))).unwrap();
        std::fs::write(
            bin.join("chown"),
            format!(
                "#!/bin/sh\nprintf '%s\\000' \"$@\" >> {}\nexec {} \"$@\"\n",
                quote(&calls),
                quote(&chown)
            ),
        )
        .unwrap();
        for name in ["cp", "chown"] {
            std::fs::set_permissions(bin.join(name), std::fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        let metadata = |path: &Path| {
            let metadata = std::fs::metadata(path).unwrap();
            (
                metadata.uid(),
                metadata.gid(),
                metadata.mode(),
                metadata.mtime(),
                metadata.mtime_nsec(),
            )
        };
        let checkout_before = metadata(&checkout);
        let sentinel_before = metadata(&sentinel);
        assert!(!home.join(".seeded").exists());
        assert!(
            home.is_dir() && checkout.is_dir(),
            "bind-parent premise must precede setup"
        );
        // SAFETY: these calls have no pointer arguments and only read identity.
        let uid = unsafe { libc::geteuid() };
        let gid = unsafe { libc::getegid() };
        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .unwrap();
        let output = std::process::Command::new("bash")
            .args([
                "-c",
                INITIALIZE,
                "--",
                "existing",
                &uid.to_string(),
                &gid.to_string(),
            ])
            .arg(&home)
            .arg(&checkout)
            .args(["no", "no", "yes"])
            .env("PATH", path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read(home.join(".seeded")).unwrap(),
            b"skeleton configuration"
        );
        assert_eq!(metadata(&home).0, uid);
        assert_eq!(metadata(&home).1, gid);
        assert_eq!(metadata(&home.join(".seeded")).0, uid);
        assert_eq!(metadata(&home.join(".seeded")).1, gid);
        std::fs::write(home.join("writable"), b"selected account can write").unwrap();
        assert_eq!(metadata(&checkout), checkout_before);
        assert_eq!(metadata(&sentinel), sentinel_before);
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"owned checkout content");
        assert!(!checkout.join("forbidden").exists());
        let bytes = std::fs::read(calls).unwrap();
        let args: Vec<_> = bytes
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .map(|arg| std::str::from_utf8(arg).unwrap())
            .collect();
        assert_eq!(
            &args[..3],
            ["--", &format!("{uid}:{gid}"), home.to_str().unwrap()]
        );
        assert_eq!(&args[3..6], ["-hR", "--", &format!("{uid}:{gid}")]);
        assert_eq!(
            args.len(),
            7,
            "recursive chown is confined to one private staging tree"
        );
        assert!(!Path::new(args[6]).starts_with(&home));
    }

    /// A name collision alone supplies no account under the selected rules;
    /// renaming that image user or inventing a uid would change the policy.
    #[test]
    fn name_only_collision_requires_a_usable_home_owner() {
        let mut facts = image();
        facts.users[0].account.name = host().name;
        assert!(facts.choose(&host()).is_err());
        facts.home_owner = Some(1000);
        assert_eq!(facts.choose(&host()).unwrap().account.uid, 1000);
        facts.groups.push((host().name, 1234));
        facts.users[0].account.name = "ubuntu".into();
        assert!(
            facts
                .choose(&host())
                .unwrap_err()
                .to_string()
                .contains("group name")
        );
    }

    /// Framing and numeric fields are required facts, not permissive hints that
    /// may silently send an unreadable image down a fallback branch.
    #[test]
    fn inspection_rejects_incomplete_and_malformed_databases() {
        let facts = ImageAccounts::parse(
            b"ubuntu:x:1000:1000::/home/ubuntu:/bin/bash\n\0ubuntu:x:1000:\n\0missing\n\0yes\n",
        )
        .unwrap();
        assert_eq!(facts.users, image().users);
        assert!(ImageAccounts::parse(b"passwd only").is_err());
        assert!(ImageAccounts::parse(b"bad\0ubuntu:x:1000:\n\0missing\0yes\n").is_err());
        assert!(ImageAccounts::parse(b"\0bad\0missing\0").is_err());
        assert!(ImageAccounts::parse(b"\0\0unknown\0").is_err());
        assert!(ImageAccounts::parse(b"\0\0missing\0yes\n").is_err());
        assert!(ImageAccounts::parse(b"\0\0missing\0unknown\n").is_err());
    }
}
