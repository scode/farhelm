//! Folder operations through held descriptors, for the locked install and
//! uninstall work (the design's section 3, "Changes go through held
//! folders").
//!
//! Finder does not take Farhelm's locks, so the app can be moved away and
//! another put in its place at any moment. Everything inside the app is
//! therefore reached relative to folders opened once and checked, never by
//! path: if the app moves on the same disk meanwhile, the work follows the
//! app that was checked. Every open refuses to follow a symbolic link.

use super::full_sync;
use rustix::fs::{AtFlags, Mode, OFlags};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

/// What sits at a name inside a held folder, without following links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Absent,
    Folder,
    File,
    Symlink,
    /// A device, socket or FIFO: never something an installer wrote.
    Other,
}

/// Classify `name` inside `dir`.
pub fn entry_kind(dir: BorrowedFd<'_>, name: &str) -> io::Result<EntryKind> {
    match rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            let kind = rustix::fs::FileType::from_raw_mode(stat.st_mode);
            Ok(match kind {
                rustix::fs::FileType::Directory => EntryKind::Folder,
                rustix::fs::FileType::RegularFile => EntryKind::File,
                rustix::fs::FileType::Symlink => EntryKind::Symlink,
                _ => EntryKind::Other,
            })
        }
        Err(error) if error == rustix::io::Errno::NOENT => Ok(EntryKind::Absent),
        Err(error) => Err(error.into()),
    }
}

/// The device and inode of `name` inside `dir`, without following a link:
/// what identifies an entry across a rename. `None` when nothing is there.
// The casts are no-ops on Linux and needed on macOS, whose `st_dev` is a
// signed 32-bit number.
#[allow(clippy::unnecessary_cast)]
pub fn identity_at(dir: BorrowedFd<'_>, name: &str) -> io::Result<Option<(u64, u64)>> {
    match rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Ok(Some((stat.st_dev as u64, stat.st_ino as u64))),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Whether `name` inside `dir` is a folder or a link that leads to one.
/// The one place a link is followed: only to refuse it.
pub fn resolves_to_folder(dir: BorrowedFd<'_>, name: &str) -> bool {
    rustix::fs::statat(dir, name, AtFlags::empty()).is_ok_and(|stat| {
        rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::Directory
    })
}

/// Open the folder `name` inside `dir`, refusing a symbolic link. `None`
/// when nothing is there.
pub fn open_folder(dir: BorrowedFd<'_>, name: &str) -> io::Result<Option<OwnedFd>> {
    match rustix::fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(folder) => Ok(Some(folder)),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Open the folder `name` inside `dir`, creating it (mode 0755) first if it
/// is missing.
pub fn open_or_create_folder(dir: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd> {
    match rustix::fs::mkdirat(dir, name, Mode::from_raw_mode(0o755)) {
        Ok(()) => {}
        Err(error) if error == rustix::io::Errno::EXIST => {}
        Err(error) => return Err(error.into()),
    }
    open_folder(dir, name)?.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
}

/// Open the regular file `name` inside `dir` for reading, refusing a
/// symbolic link. `None` when nothing is there.
pub fn open_file(dir: BorrowedFd<'_>, name: &str) -> io::Result<Option<File>> {
    match rustix::fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => {
            let file = File::from(fd);
            if !file.metadata()?.is_file() {
                return Err(io::Error::other(format!("{name} is not a regular file")));
            }
            Ok(Some(file))
        }
        Err(error) if error == rustix::io::Errno::NOENT => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Read a small regular file inside `dir` (at most `limit` bytes; more is
/// an error). `None` when it is absent.
pub fn read_small(dir: BorrowedFd<'_>, name: &str, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let Some(file) = open_file(dir, name)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::other(format!(
            "{name} is larger than {limit} bytes"
        )));
    }
    Ok(Some(bytes))
}

/// The prefix of every temporary file the installer puts inside the app.
/// Older installers clean up regular files with this prefix and older
/// uninstallers accept them, so it is the only temporary shape allowed
/// inside the app.
pub const TEMPORARY_PREFIX: &str = ".farhelm-new.";

/// Write `contents` to `name` inside `dir` with `mode`, durably and
/// atomically: a `.farhelm-new.*` file beside it is written, flushed and
/// then renamed over `name`. A reader sees the old file or the new one,
/// never a partial one, and a program's code signature is never rewritten
/// in place. The folder itself is not flushed here; callers flush it at the
/// points the write order depends on.
pub fn replace_file(
    dir: BorrowedFd<'_>,
    name: &str,
    contents: &mut dyn Read,
    mode: u32,
) -> io::Result<()> {
    let temporary = format!("{TEMPORARY_PREFIX}{}.{name}", uuid::Uuid::new_v4().simple());
    let written = (|| {
        let fd = rustix::fs::openat(
            dir,
            temporary.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        let mut file = File::from(fd);
        io::copy(contents, &mut file)?;
        file.flush()?;
        // Through std, whose mode type is the same on every platform
        // (rustix's is 16 bits on macOS).
        file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(mode))?;
        full_sync(file.as_fd())?;
        rustix::fs::renameat(dir, temporary.as_str(), dir, name)?;
        Ok::<(), io::Error>(())
    })();
    if written.is_err() {
        let _ = rustix::fs::unlinkat(dir, temporary.as_str(), AtFlags::empty());
    }
    written
}

/// [`replace_file`] with the contents of the file at `source`.
pub fn replace_file_from(
    dir: BorrowedFd<'_>,
    name: &str,
    source: &Path,
    mode: u32,
) -> io::Result<()> {
    let mut source = File::open(source)?;
    replace_file(dir, name, &mut source, mode)
}

/// Whether the regular file `name` inside `dir` holds exactly the bytes of
/// the file at `source`.
pub fn same_contents(dir: BorrowedFd<'_>, name: &str, source: &Path) -> io::Result<bool> {
    let Some(mut ours) = open_file(dir, name)? else {
        return Ok(false);
    };
    let mut theirs = File::open(source)?;
    let (mut a, mut b) = (Vec::new(), Vec::new());
    ours.read_to_end(&mut a)?;
    theirs.read_to_end(&mut b)?;
    Ok(a == b)
}

/// Remove the regular, non-link `.farhelm-new.*` files a killed installer
/// left inside `dir`, exactly as older installers do. Anything else with
/// that prefix (a link, a folder) is not the installer's and stays.
pub fn remove_temporaries(dir: BorrowedFd<'_>) -> io::Result<()> {
    for name in list(dir)? {
        if name.starts_with(TEMPORARY_PREFIX) && entry_kind(dir, &name)? == EntryKind::File {
            rustix::fs::unlinkat(dir, name.as_str(), AtFlags::empty())?;
        }
    }
    Ok(())
}

/// The names inside `dir` (not `.` or `..`), skipping names that are not
/// valid UTF-8 (the installer never writes one).
pub fn list(dir: BorrowedFd<'_>) -> io::Result<Vec<String>> {
    // A fresh descriptor, so iterating does not move a position the caller
    // shares.
    let fd = rustix::fs::openat(
        dir,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let mut names = Vec::new();
    for entry in rustix::fs::Dir::read_from(&fd)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().to_str() else {
            continue;
        };
        if name != "." && name != ".." {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

/// Remove the folder `name` inside `dir` and everything in it, without
/// following any link inside. Absent is already removed.
pub fn remove_tree(dir: BorrowedFd<'_>, name: &str) -> io::Result<()> {
    match entry_kind(dir, name)? {
        EntryKind::Absent => Ok(()),
        EntryKind::Folder => {
            let Some(folder) = open_folder(dir, name)? else {
                return Ok(());
            };
            for child in list(folder.as_fd())? {
                remove_tree(folder.as_fd(), &child)?;
            }
            rustix::fs::unlinkat(dir, name, AtFlags::REMOVEDIR)?;
            Ok(())
        }
        _ => Ok(rustix::fs::unlinkat(dir, name, AtFlags::empty())?),
    }
}

/// Flush every file and folder under the held folder `dir` (and `dir`
/// itself), so a tree built under a temporary name is on disk before it is
/// renamed into place.
pub fn sync_tree(dir: BorrowedFd<'_>) -> io::Result<()> {
    for name in list(dir)? {
        match entry_kind(dir, &name)? {
            EntryKind::Folder => {
                if let Some(child) = open_folder(dir, &name)? {
                    sync_tree(child.as_fd())?;
                }
            }
            EntryKind::File => {
                if let Some(file) = open_file(dir, &name)? {
                    full_sync(file.as_fd())?;
                }
            }
            _ => {}
        }
    }
    full_sync(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(path: &Path) -> OwnedFd {
        rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .expect("open a fixture folder")
    }

    /// A replaced file is either the old one or the complete new one, has
    /// the requested mode, and leaves no temporary behind; a folder or link
    /// is never followed when classifying or opening.
    #[farhelm_testtrace::test]
    fn files_are_replaced_whole_and_links_are_never_followed() {
        let root = tempfile::tempdir().expect("temp dir");
        let dir = open(root.path());
        std::fs::write(root.path().join("target"), b"old").unwrap();
        replace_file(dir.as_fd(), "target", &mut &b"new contents"[..], 0o644).expect("replace");
        assert_eq!(
            std::fs::read(root.path().join("target")).unwrap(),
            b"new contents"
        );
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.path().join("target"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
        assert_eq!(list(dir.as_fd()).unwrap(), vec!["target".to_string()]);

        std::os::unix::fs::symlink(root.path(), root.path().join("link")).unwrap();
        assert_eq!(entry_kind(dir.as_fd(), "link").unwrap(), EntryKind::Symlink);
        assert!(
            open_folder(dir.as_fd(), "link").is_err(),
            "a link to a folder is refused"
        );
        assert!(
            open_file(dir.as_fd(), "link").is_err(),
            "a link is never read through"
        );
        assert_eq!(
            entry_kind(dir.as_fd(), "missing").unwrap(),
            EntryKind::Absent
        );
    }

    /// Cleanup removes only what an installer could have left: regular
    /// `.farhelm-new.*` files, not links or folders that merely share the
    /// prefix; and tree removal never follows a link out of the tree.
    #[farhelm_testtrace::test]
    fn cleanup_removes_only_installer_temporaries_and_stays_inside_the_tree() {
        let root = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("temp dir");
        std::fs::write(outside.path().join("keep"), b"keep").unwrap();
        let dir = open(root.path());
        std::fs::write(root.path().join(".farhelm-new.1.x"), b"half").unwrap();
        std::os::unix::fs::symlink("/nonexistent", root.path().join(".farhelm-new.2.link"))
            .unwrap();
        std::fs::create_dir(root.path().join(".farhelm-new.3.dir")).unwrap();
        remove_temporaries(dir.as_fd()).expect("cleanup");
        let mut names = list(dir.as_fd()).unwrap();
        names.sort();
        assert_eq!(names, [".farhelm-new.2.link", ".farhelm-new.3.dir"]);

        std::fs::create_dir_all(root.path().join("tree/inner")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("tree/inner/out")).unwrap();
        remove_tree(dir.as_fd(), "tree").expect("remove the tree");
        assert!(!root.path().join("tree").exists());
        assert!(
            outside.path().join("keep").exists(),
            "nothing outside the tree is touched"
        );
    }
}
