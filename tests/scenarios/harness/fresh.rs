//! Fresh directories under the system's temporary directory, removed when dropped.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct FreshDirectory {
    path: PathBuf,
}

impl FreshDirectory {
    /// Creates an empty directory nothing else uses. Its path is canonical, so that it
    /// compares equal to the paths Git reports.
    pub fn create() -> FreshDirectory {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let temporary = std::env::temp_dir();
        let temporary = fs::canonicalize(&temporary).unwrap_or_else(|cause| {
            panic!("the temporary directory {}: {cause}", temporary.display())
        });
        loop {
            let name = format!(
                "git-dupe-checks-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = temporary.join(name);
            match fs::create_dir(&path) {
                Ok(()) => return FreshDirectory { path },
                // Left by an earlier process that had this one's identifier and was killed.
                Err(cause) if cause.kind() == ErrorKind::AlreadyExists => continue,
                Err(cause) => panic!("cannot create {}: {cause}", path.display()),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Creates a drop-guarded directory below an explicit base, or returns `None`
    /// when that base is unavailable or cannot hold a fresh directory.
    pub fn create_in(base: &Path) -> Option<FreshDirectory> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let base = fs::canonicalize(base).ok()?;
        loop {
            let path = base.join(format!(
                "git-dupe-checks-base-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Some(FreshDirectory { path }),
                Err(cause) if cause.kind() == ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
    }
}

impl Drop for FreshDirectory {
    fn drop(&mut self) {
        if let Err(cause) = fs::remove_dir_all(&self.path) {
            // Not a panic: this also runs while a failed check unwinds.
            eprintln!("cannot remove {}: {cause}", self.path.display());
        }
    }
}
