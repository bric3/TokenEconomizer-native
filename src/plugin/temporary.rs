//! Permission-aware temporary-directory cleanup, with descriptor-relative Unix traversal.
use std::{fs, io, path::Path};

pub(crate) fn remove_tree(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::other("Cannot call rmtree on a symbolic link"));
    }
    fs::remove_dir_all(path)
}

pub(crate) fn remove(path: &Path, ignore_errors: bool) -> io::Result<()> {
    let result = remove_tree(path);
    #[cfg(unix)]
    let result = match result {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            unix::remove(path, ignore_errors)
        }
        result => result,
    };
    match result {
        Err(error) if ignore_errors || error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(unix)]
mod unix {
    use std::{
        ffi::{CStr, CString},
        io,
        mem::MaybeUninit,
        os::{
            fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
            unix::ffi::OsStrExt,
        },
        path::Path,
        ptr::NonNull,
    };

    #[cfg(any(target_os = "solaris", target_os = "illumos"))]
    use libc::___errno as errno;
    #[cfg(any(target_os = "android", target_os = "netbsd", target_os = "openbsd"))]
    use libc::__errno as errno;
    #[cfg(not(any(
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "android",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "aix"
    )))]
    use libc::__errno_location as errno;
    #[cfg(any(target_vendor = "apple", target_os = "freebsd"))]
    use libc::__error as errno;
    #[cfg(target_os = "aix")]
    use libc::_Errno as errno;

    fn checked(value: libc::c_int) -> io::Result<()> {
        if value < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    fn stat(parent: RawFd, name: &CStr) -> io::Result<libc::stat> {
        let mut metadata = MaybeUninit::uninit();
        // The directory descriptor anchors name; NOFOLLOW inspects the entry itself.
        checked(unsafe {
            libc::fstatat(
                parent,
                name.as_ptr(),
                metadata.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        })?;
        Ok(unsafe { metadata.assume_init() })
    }
    fn open(parent: RawFd, name: &CStr, flags: libc::c_int) -> io::Result<OwnedFd> {
        // macOS O_SYMLINK opens the link itself; O_NOFOLLOW would reject that handle.
        let no_follow = libc::O_NOFOLLOW;
        #[cfg(target_vendor = "apple")]
        let no_follow = if flags & libc::O_SYMLINK != 0 {
            0
        } else {
            no_follow
        };
        let fd =
            unsafe { libc::openat(parent, name.as_ptr(), flags | libc::O_CLOEXEC | no_follow) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }
    fn reset(fd: RawFd) -> io::Result<()> {
        #[cfg(any(
            target_vendor = "apple",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        // Both calls operate on the opened inode, including after a rename.
        checked(unsafe { libc::fchflags(fd, 0) })?;
        checked(unsafe { libc::fchmod(fd, 0o700) })
    }
    fn reset_entry(parent: RawFd, name: &CStr) -> io::Result<()> {
        let metadata = stat(parent, name)?;
        let link = metadata.st_mode & libc::S_IFMT == libc::S_IFLNK;
        #[cfg(target_vendor = "apple")]
        let flags = libc::O_EVTONLY | libc::O_SYMLINK;
        #[cfg(not(target_vendor = "apple"))]
        let flags = libc::O_RDONLY | libc::O_NONBLOCK;
        #[cfg(not(target_vendor = "apple"))]
        if link {
            return Ok(());
        }
        #[cfg(target_vendor = "apple")]
        let _ = link;
        let fd = match open(parent, name, flags) {
            Ok(fd) => fd,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                // Repair unreadable owned entries without following a replacement symlink.
                checked(unsafe {
                    libc::fchmodat(parent, name.as_ptr(), 0o700, libc::AT_SYMLINK_NOFOLLOW)
                })?;
                open(parent, name, flags)?
            }
            Err(error) => return Err(error),
        };
        reset(fd.as_raw_fd())
    }
    fn repair(parent: RawFd, name: &CStr, root: bool) -> io::Result<()> {
        if !root {
            reset(parent)?;
        }
        reset_entry(parent, name)
    }
    fn unlink(parent: RawFd, name: &CStr, flags: libc::c_int) -> io::Result<()> {
        // unlinkat removes this directory entry and never follows its final component.
        checked(unsafe { libc::unlinkat(parent, name.as_ptr(), flags) })
    }

    struct Entries {
        stream: NonNull<libc::DIR>,
        descriptor: RawFd,
    }
    impl Entries {
        fn new(fd: OwnedFd) -> io::Result<Self> {
            let descriptor = fd.into_raw_fd();
            // The stream owns the traversal descriptor, so each level keeps one descriptor open.
            let directory = unsafe { libc::fdopendir(descriptor) };
            match NonNull::new(directory) {
                Some(stream) => Ok(Self { stream, descriptor }),
                None => {
                    let error = io::Error::last_os_error();
                    unsafe {
                        libc::close(descriptor);
                    }
                    Err(error)
                }
            }
        }
        fn next(&mut self) -> io::Result<Option<CString>> {
            loop {
                // readdir uses thread-local errno to distinguish EOF from a directory read error.
                let entry = unsafe {
                    *errno() = 0;
                    libc::readdir(self.stream.as_ptr())
                };
                if entry.is_null() {
                    let error = io::Error::last_os_error();
                    return if error.raw_os_error() == Some(0) {
                        Ok(None)
                    } else {
                        Err(error)
                    };
                }
                // The entry's NUL-terminated name remains valid until the next readdir call.
                let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
                if !matches!(name.to_bytes(), b"." | b"..") {
                    return Ok(Some(name.into()));
                }
            }
        }
    }
    impl Drop for Entries {
        fn drop(&mut self) {
            // closedir releases both the stream and its traversal descriptor.
            unsafe {
                libc::closedir(self.stream.as_ptr());
            }
        }
    }

    fn remove_entry(parent: RawFd, name: &CStr, root: bool, ignore_errors: bool) -> io::Result<()> {
        let metadata = match stat(parent, name) {
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied && !root => {
                reset(parent)?;
                stat(parent, name)?
            }
            result => result?,
        };
        if metadata.st_mode & libc::S_IFMT != libc::S_IFDIR {
            if root {
                return Err(io::Error::other("Cannot call rmtree on a symbolic link"));
            }
            return match unlink(parent, name, 0) {
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    repair(parent, name, false)?;
                    unlink(parent, name, 0)
                }
                result => result,
            };
        }
        let directory = match open(parent, name, libc::O_RDONLY | libc::O_DIRECTORY) {
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                repair(parent, name, root)?;
                open(parent, name, libc::O_RDONLY | libc::O_DIRECTORY)?
            }
            result => result?,
        };
        let mut entries = Entries::new(directory)?;
        while let Some(child) = entries.next()? {
            let result = remove_entry(entries.descriptor, &child, false, ignore_errors);
            if let Err(error) = result
                && !ignore_errors
                && error.kind() != io::ErrorKind::NotFound
            {
                return Err(error);
            }
        }
        match unlink(parent, name, libc::AT_REMOVEDIR) {
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                if !root {
                    reset(parent)?;
                }
                reset(entries.descriptor)?;
                unlink(parent, name, libc::AT_REMOVEDIR)
            }
            result => result,
        }
    }

    pub(super) fn remove(path: &Path, ignore_errors: bool) -> io::Result<()> {
        let name = CString::new(path.as_os_str().as_bytes())?;
        remove_entry(libc::AT_FDCWD, &name, true, ignore_errors)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::plugin::sidecall::temporary_directory;
        use std::{
            fs::{self, File},
            os::unix::fs::{PermissionsExt, symlink},
        };

        #[test]
        fn repairing_an_unreadable_entry_preserves_its_contents() {
            let inherited =
                std::env::var_os("TE_CLEANUP_TEST_CHILD").filter(|_| cfg!(target_os = "linux"));
            let root = inherited
                .clone()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    temporary_directory("te-reset-", &std::env::temp_dir()).unwrap()
                });
            let child = root.join("child");
            if inherited.is_some() {
                let prefix = format!("te-reset-{}-", unsafe { libc::getppid() });
                assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
                assert!(
                    root.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(&prefix)
                );
                assert!(fs::symlink_metadata(&root).unwrap().file_type().is_dir());
            }
            if inherited.is_none() {
                fs::create_dir(&child).unwrap();
                fs::write(child.join("data"), b"keep").unwrap();
                fs::set_permissions(&child, fs::Permissions::from_mode(0o000)).unwrap();
            }
            #[cfg(target_os = "linux")]
            if inherited.is_none() && unsafe { libc::geteuid() } == 0 {
                use std::{os::unix::process::CommandExt, process::Command};
                for path in [&root, &child, &child.join("data")] {
                    let name = CString::new(path.as_os_str().as_bytes()).unwrap();
                    // The builder's root user gives only these owned fixture paths to its test child.
                    checked(unsafe { libc::chown(name.as_ptr(), 65534, 65534) }).unwrap();
                }
                let name = format!(
                    "{}::repairing_an_unreadable_entry_preserves_its_contents",
                    module_path!().split_once("::").unwrap().1
                );
                let status = Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", &name])
                    .env("TE_CLEANUP_TEST_CHILD", &root)
                    .uid(65534)
                    .gid(65534)
                    .status()
                    .unwrap();
                assert!(
                    status.success(),
                    "unprivileged permission-repair check failed"
                );
                return;
            }
            let parent = File::open(&root).unwrap();
            reset_entry(parent.as_raw_fd(), c"child").unwrap();
            assert_eq!(
                fs::metadata(&child).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(fs::read(child.join("data")).unwrap(), b"keep");
            super::super::remove_tree(&root).unwrap();
        }

        #[test]
        fn repairing_a_link_keeps_its_target_permissions_unchanged() {
            let root = temporary_directory("te-link-", &std::env::temp_dir()).unwrap();
            let target = root.join("target");
            fs::write(&target, b"keep").unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o400)).unwrap();
            symlink(&target, root.join("link")).unwrap();
            let parent = File::open(&root).unwrap();
            reset_entry(parent.as_raw_fd(), c"link").unwrap();
            assert_eq!(
                fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o400
            );
            assert_eq!(fs::read(&target).unwrap(), b"keep");
            super::super::remove_tree(&root).unwrap();
        }

        #[test]
        fn replaced_root_link_obeys_the_cleanup_error_policy() {
            let root = temporary_directory("te-root-link-", &std::env::temp_dir()).unwrap();
            let target = root.join("target");
            fs::create_dir(&target).unwrap();
            fs::write(target.join("data"), b"keep").unwrap();
            let link = root.join("replaced-root");
            symlink(&target, &link).unwrap();
            assert!(super::super::remove(&link, false).is_err());
            super::super::remove(&link, true).unwrap();
            assert!(
                fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(fs::read(target.join("data")).unwrap(), b"keep");
            super::super::remove_tree(&root).unwrap();
        }
    }
}
