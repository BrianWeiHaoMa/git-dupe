//! A release on Windows, where none is built (S14, S21): Git for Windows's `MinGit` archive of
//! the listed word, as `MINGIT` in `releases.rs` names it, never a name derived from the word,
//! downloaded with `curl.exe`, checked against the digest the repository pins, never one the
//! release page gives at run time, and unpacked with Windows's own `tar.exe`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{MARKER, Store, because, remove_tree, run};
use crate::harness::releases::{MinGit, row};
use crate::harness::sha256;

/// Where Git for Windows publishes its releases, each under its tag.
pub(super) const OFFICIAL_RELEASES: &str =
    "https://github.com/git-for-windows/git/releases/download";

impl Store {
    pub(super) fn unpack(
        &self,
        table: &[MinGit],
        versions: &[&str],
        work: &Path,
    ) -> Result<(), String> {
        for word in versions {
            let pinned = row(table, word)
                .ok_or_else(|| format!("no MinGit archive is pinned for {word}"))?;
            self.unpack_release(word, pinned, work)?;
        }
        Ok(())
    }

    fn unpack_release(&self, word: &str, pinned: &MinGit, work: &Path) -> Result<(), String> {
        let asset = pinned.asset();
        let url = format!("{}/{}/{asset}", self.source, pinned.tag());
        let zip = work.join(&asset);
        run(Command::new("curl.exe")
            .args(["-fsSL", "--max-time", "600", "-o"])
            .arg(&zip)
            .arg(&url))?;
        let content = fs::read(&zip).map_err(because("read", &zip))?;
        let actual = sha256::hex_digest(&content);
        if actual != pinned.sha256 {
            return Err(format!(
                "{url} has the SHA-256 {actual}, where the repository pins {}",
                pinned.sha256
            ));
        }

        let directory = self.release_directory(word);
        remove_tree(&directory)?;
        fs::create_dir(&directory).map_err(because("create", &directory))?;
        run(Command::new(windows_tar()?)
            .arg("-xf")
            .arg(&zip)
            .arg("-C")
            .arg(&directory))?;

        let marker = directory.join(MARKER);
        fs::write(&marker, format!("{actual}\n")).map_err(because("write", &marker))?;
        fs::remove_file(&zip).map_err(because("remove", &zip))
    }
}

/// Windows's own `tar.exe` by its full path, because a GNU `tar` on `PATH` reads `C:\…` as a
/// host (`Composition/The checks`).
pub(super) fn windows_tar() -> Result<PathBuf, String> {
    let root = std::env::var_os("SystemRoot").ok_or("SystemRoot is not set")?;
    Ok(Path::new(&root).join("System32").join("tar.exe"))
}
