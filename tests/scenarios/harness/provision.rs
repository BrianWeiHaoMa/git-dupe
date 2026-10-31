//! Obtaining the Git releases: each is put into `gits/git-<version>/` at the repository root
//! when absent, and reused when present. Built from its official source release on Linux and
//! macOS, which needs `curl`, `tar` with `xz`, `make`, a C compiler, and the zlib headers;
//! unpacked from its pinned `MinGit` archive on Windows (`provision/mingit.rs`), which needs
//! `curl.exe` and `tar.exe`. The network only while a release is absent.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::platform;
use super::releases::{MINGIT, MinGit};
use super::sha256;

mod mingit;

/// Where Git's source releases are published, their digests beside them.
const OFFICIAL_SOURCE: &str = "https://mirrors.edge.kernel.org/pub/software/scm/git";
const DIGEST_LIST: &str = "sha256sums.asc";

/// The knobs of the build, the same for every release: none is added, dropped, or made
/// to depend on the release to get a build through.
const KNOBS: [&str; 9] = [
    "RUNTIME_PREFIX=YesPlease",
    "NO_GETTEXT=1",
    "NO_TCLTK=1",
    "NO_PERL=1",
    "NO_PYTHON=1",
    "NO_CURL=1",
    "NO_EXPAT=1",
    "NO_OPENSSL=1",
    "NO_RUST=1",
];

/// Written into a release's directory last, once `make … install` or the unpacking has
/// succeeded, and holding the digest of the tarball or zip it came from. A directory without
/// it is a release that did not finish: obtained again, never used.
const MARKER: &str = "complete";

/// A directory of releases and the place they come from.
pub struct Store {
    directory: PathBuf,
    source: String,
    /// The archives a release is unpacked from, where none is built: Windows's (S21).
    pinned: Option<&'static [MinGit]>,
}

impl Store {
    /// `gits/` at the repository root, filled by this platform's procedure (F4).
    pub fn of_the_repository() -> Store {
        let unpacked = platform::word() == "windows";
        Store {
            directory: Path::new(env!("CARGO_MANIFEST_DIR")).join("gits"),
            source: if unpacked {
                mingit::OFFICIAL_RELEASES
            } else {
                OFFICIAL_SOURCE
            }
            .to_string(),
            pinned: unpacked.then_some(&MINGIT),
        }
    }

    /// The directory a release is installed into, `platform::release_programs` naming what
    /// in it a scenario runs.
    pub fn release_directory(&self, version: &str) -> PathBuf {
        self.directory.join(format!("git-{version}"))
    }

    /// The releases among `versions` that are not present.
    pub fn absent<'v>(&self, versions: &[&'v str]) -> Vec<&'v str> {
        let present = |version: &str| self.release_directory(version).join(MARKER).is_file();
        versions
            .iter()
            .copied()
            .filter(|version| !present(version))
            .collect()
    }

    /// Makes every release in `versions` present, obtaining the absent ones one after
    /// another. A release that is present is not downloaded, rebuilt, or touched. Safe
    /// under parallel test threads and under check commands started together.
    pub fn provide(&self, versions: &[&str]) -> Result<(), String> {
        if self.absent(versions).is_empty() {
            return Ok(());
        }
        fs::create_dir_all(&self.directory).map_err(because("create", &self.directory))?;

        // One at a time in this directory; held until this function returns. Never truncated,
        // which a lock held through another handle refuses on Windows (S16).
        let lock_path = self.directory.join("lock");
        let lock = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(because("create", &lock_path))?;
        lock.lock().map_err(because("lock", &lock_path))?;

        // Another run may have obtained some of them while this one waited for the lock.
        let absent = self.absent(versions);
        if absent.is_empty() {
            return Ok(());
        }

        // Downloads and extracted sources live here meanwhile and are gone afterward. The
        // lock is held, so whatever is here was left by a run that was killed.
        let work = self.directory.join("work");
        remove_tree(&work)?;
        fs::create_dir(&work).map_err(because("create", &work))?;
        let obtained = match self.pinned {
            Some(table) => self.unpack(table, &absent, &work),
            None => self.build(&absent, &work),
        };
        let cleared = remove_tree(&work);
        obtained.and(cleared)
    }

    fn build(&self, versions: &[&str], work: &Path) -> Result<(), String> {
        let list_path = work.join(DIGEST_LIST);
        self.download(DIGEST_LIST, &list_path)?;
        let list = fs::read(&list_path).map_err(because("read", &list_path))?;
        for version in versions {
            self.build_release(version, work, &list)?;
        }
        Ok(())
    }

    fn build_release(&self, version: &str, work: &Path, list: &[u8]) -> Result<(), String> {
        let source = &self.source;
        let tarball_name = format!("git-{version}.tar.xz");
        let listed = listed_digest(list, &tarball_name).ok_or_else(|| {
            format!("{DIGEST_LIST} at {source} has no SHA-256 entry for {tarball_name}")
        })?;
        let tarball = work.join(&tarball_name);
        self.download(&tarball_name, &tarball)?;
        let content = fs::read(&tarball).map_err(because("read", &tarball))?;
        let actual = sha256::hex_digest(&content);
        if actual != listed {
            return Err(format!(
                "{tarball_name} from {source} has the SHA-256 {actual}, \
                 where {DIGEST_LIST} lists {listed}"
            ));
        }
        run(Command::new("tar")
            .arg("-xJf")
            .arg(&tarball)
            .arg("-C")
            .arg(work))?;

        let directory = self.release_directory(version);
        remove_tree(&directory)?;
        let mut prefix = OsString::from("prefix=");
        prefix.push(&directory);
        let jobs = std::thread::available_parallelism().map_or(1, |count| count.get());
        let extracted = work.join(format!("git-{version}"));
        let mut make = Command::new("make");
        make.arg("-C")
            .arg(&extracted)
            .arg(format!("-j{jobs}"))
            .arg(prefix)
            .args(KNOBS)
            .arg("install");
        // A release's build asks the `git` on `PATH` about the repository it is built in.
        // The ceiling keeps that from finding the repository that holds this directory,
        // and no variable of the caller's names another.
        for (name, _) in std::env::vars_os() {
            if name.as_encoded_bytes().starts_with(b"GIT_") {
                make.env_remove(name);
            }
        }
        make.env("GIT_CEILING_DIRECTORIES", work);
        run(&mut make)?;

        let marker = directory.join(MARKER);
        fs::write(&marker, format!("{actual}\n")).map_err(because("write", &marker))?;
        remove_tree(&extracted)?;
        fs::remove_file(&tarball).map_err(because("remove", &tarball))
    }

    fn download(&self, name: &str, to: &Path) -> Result<(), String> {
        run(Command::new("curl")
            .args(["-fsSL", "--max-time", "600", "-o"])
            .arg(to)
            .arg(format!("{}/{name}", self.source)))
    }
}

/// The digest the list gives for `file_name`: 64 hexadecimal digits, then the name, on a
/// line of its own. The list is a signed message; its other lines are skipped.
fn listed_digest(list: &[u8], file_name: &str) -> Option<String> {
    String::from_utf8_lossy(list).lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let (digest, name, more) = (fields.next()?, fields.next()?, fields.next());
        let is_digest = digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit());
        (is_digest && name == file_name && more.is_none()).then(|| digest.to_ascii_lowercase())
    })
}

/// Runs a tool to its end; a failure carries the command and the end of what it said.
fn run(command: &mut Command) -> Result<(), String> {
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|cause| format!("cannot run {command:?}: {cause}"))?;
    if output.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = said.lines().collect();
    let tail = lines[lines.len().saturating_sub(30)..].join("\n");
    Err(format!("{command:?}: {}\n{tail}", output.status))
}

fn remove_tree(path: &Path) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Err(cause) if cause.kind() != ErrorKind::NotFound => Err(because("remove", path)(cause)),
        _ => Ok(()),
    }
}

fn because(act: &str, path: &Path) -> impl Fn(std::io::Error) -> String {
    move |cause| format!("cannot {act} {}: {cause}", path.display())
}

#[cfg(test)]
mod tests;
