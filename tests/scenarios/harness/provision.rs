//! Obtaining the Git releases: each is built from its official source release into
//! `gits/git-<version>/` at the repository root when absent, and reused when present.
//! Needs `curl`, `tar` with `xz`, `make`, a C compiler, and the zlib headers, and the
//! network only while a release is absent.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::ErrorKind;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::sha256;

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

/// Written into a release's directory last, once `make … install` has succeeded, and
/// holding the digest of the tarball it was built from. A directory without it is a
/// build that did not finish: rebuilt, never used.
const MARKER: &str = "complete";

/// A directory of built releases and the place their sources come from.
pub struct Store {
    directory: PathBuf,
    source: String,
}

impl Store {
    /// `gits/` at the repository root, filled from the official source releases.
    pub fn of_the_repository() -> Store {
        Store {
            directory: Path::new(env!("CARGO_MANIFEST_DIR")).join("gits"),
            source: OFFICIAL_SOURCE.to_string(),
        }
    }

    /// The prefix a release is installed into; its `bin` holds `git`.
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

    /// Makes every release in `versions` present, building the absent ones one after
    /// another. A release that is present is not downloaded, rebuilt, or touched. Safe
    /// under parallel test threads and under check commands started together.
    pub fn provide(&self, versions: &[&str]) -> Result<(), String> {
        if self.absent(versions).is_empty() {
            return Ok(());
        }
        fs::create_dir_all(&self.directory).map_err(because("create", &self.directory))?;

        // One build at a time in this directory; held until this function returns.
        let lock_path = self.directory.join("lock");
        let lock = File::create(&lock_path).map_err(because("create", &lock_path))?;
        lock.lock().map_err(because("lock", &lock_path))?;

        // Another run may have built some of them while this one waited for the lock.
        let absent = self.absent(versions);
        if absent.is_empty() {
            return Ok(());
        }

        // Downloads and extracted sources live here while building and are gone afterward.
        // The lock is held, so whatever is here was left by a run that was killed.
        let work = self.directory.join("work");
        remove_tree(&work)?;
        fs::create_dir(&work).map_err(because("create", &work))?;
        let built = self.build(&absent, &work);
        let cleared = remove_tree(&work);
        built.and(cleared)
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
            if name.as_bytes().starts_with(b"GIT_") {
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
mod tests {
    use super::*;
    use crate::harness::fresh::FreshDirectory;

    /// The one release the stand-in source publishes. It is not a Git release.
    const VERSION: &str = "0.0.0";

    /// A stand-in for the official source, read through `file://`: one source tarball
    /// whose `make install` writes a `bin/git` recording the knobs it was given and
    /// adds a line to a tally of builds. The digest list is each check's to publish.
    struct Published {
        root: FreshDirectory,
    }

    impl Published {
        fn new() -> Published {
            let root = FreshDirectory::create();
            let staged = root.path().join(format!("staged/git-{VERSION}"));
            fs::create_dir_all(&staged).unwrap();
            fs::create_dir(root.path().join("source")).unwrap();
            let recorded: Vec<String> = KNOBS
                .iter()
                .map(|knob| format!("$({})", knob.split('=').next().unwrap()))
                .collect();
            let makefile = format!(
                "install:\n\
                 \tmkdir -p '$(prefix)/bin'\n\
                 \techo '{}' >'$(prefix)/bin/git'\n\
                 \techo built >>'{}'\n",
                recorded.join(" "),
                root.path().join("tally").display()
            );
            fs::write(staged.join("Makefile"), makefile).unwrap();
            let published = Published { root };
            run(Command::new("tar")
                .arg("-cJf")
                .arg(published.tarball())
                .arg("-C")
                .arg(published.root.path().join("staged"))
                .arg(format!("git-{VERSION}")))
            .unwrap();
            published
        }

        fn tarball(&self) -> PathBuf {
            self.root
                .path()
                .join(format!("source/git-{VERSION}.tar.xz"))
        }

        fn tarball_digest(&self) -> String {
            sha256::hex_digest(&fs::read(self.tarball()).unwrap())
        }

        fn publish_list(&self, lines: &str) {
            let list = format!(
                "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA256\n\n{lines}\
                 -----BEGIN PGP SIGNATURE-----\n\nAAAA\n-----END PGP SIGNATURE-----\n"
            );
            fs::write(self.root.path().join("source").join(DIGEST_LIST), list).unwrap();
        }

        fn publish_the_tarballs_digest(&self) {
            self.publish_list(&format!(
                "{}  git-{VERSION}.tar.xz\n",
                self.tarball_digest()
            ));
        }

        fn store(&self) -> Store {
            Store {
                directory: self.root.path().join("gits"),
                source: format!("file://{}/source", self.root.path().display()),
            }
        }

        fn builds(&self) -> usize {
            match fs::read_to_string(self.root.path().join("tally")) {
                Ok(tally) => tally.lines().count(),
                Err(cause) if cause.kind() == ErrorKind::NotFound => 0,
                Err(cause) => panic!("the tally of builds: {cause}"),
            }
        }

        fn nothing_was_built(&self) {
            let store = self.store();
            assert_eq!(self.builds(), 0);
            assert!(!store.release_directory(VERSION).exists());
            assert!(!store.directory.join("work").exists());
        }
    }

    #[test]
    fn a_release_is_built_with_the_knobs_once_and_then_left_alone() {
        let published = Published::new();
        published.publish_the_tarballs_digest();
        let store = published.store();
        assert_eq!(store.absent(&[VERSION]), [VERSION]);
        store.provide(&[VERSION]).unwrap();

        let git = store.release_directory(VERSION).join("bin/git");
        let marker = store.release_directory(VERSION).join(MARKER);
        assert_eq!(
            fs::read_to_string(&git).unwrap(),
            "YesPlease 1 1 1 1 1 1 1 1\n"
        );
        assert_eq!(
            fs::read_to_string(&marker).unwrap(),
            format!("{}\n", published.tarball_digest())
        );
        assert!(store.absent(&[VERSION]).is_empty());
        assert!(!store.directory.join("work").exists());

        // With its source gone, a present release still provides: nothing is downloaded.
        let modified = |path: &Path| fs::metadata(path).unwrap().modified().unwrap();
        let before = [modified(&git), modified(&marker)];
        fs::remove_dir_all(published.root.path().join("source")).unwrap();
        store.provide(&[VERSION]).unwrap();
        assert_eq!([modified(&git), modified(&marker)], before);
        assert_eq!(published.builds(), 1);
    }

    #[test]
    fn a_directory_without_its_marker_is_rebuilt_not_used() {
        let published = Published::new();
        published.publish_the_tarballs_digest();
        let store = published.store();
        store.provide(&[VERSION]).unwrap();

        let directory = store.release_directory(VERSION);
        fs::remove_file(directory.join(MARKER)).unwrap();
        fs::write(directory.join("half-installed"), "").unwrap();
        assert_eq!(store.absent(&[VERSION]), [VERSION]);
        store.provide(&[VERSION]).unwrap();

        assert!(directory.join(MARKER).is_file());
        assert!(!directory.join("half-installed").exists());
        assert_eq!(published.builds(), 2);
    }

    #[test]
    fn a_tarball_that_does_not_hash_to_its_listed_digest_builds_nothing() {
        let published = Published::new();
        let other = sha256::hex_digest(b"another tarball");
        published.publish_list(&format!("{other}  git-{VERSION}.tar.xz\n"));

        let cause = published.store().provide(&[VERSION]).unwrap_err();
        assert!(cause.contains(&other), "{cause}");
        assert!(cause.contains(&published.tarball_digest()), "{cause}");
        published.nothing_was_built();
    }

    #[test]
    fn a_release_the_digest_list_does_not_name_builds_nothing() {
        let published = Published::new();
        let digest = published.tarball_digest();
        // Its digest is there, but only beside other names.
        published.publish_list(&format!(
            "{digest}  git-{VERSION}.tar.gz\n\
             {digest}  git-manpages-{VERSION}.tar.xz\n\
             {digest}  git-{VERSION}.tar.xz.sign\n"
        ));

        let cause = published.store().provide(&[VERSION]).unwrap_err();
        assert!(
            cause.contains(&format!("no SHA-256 entry for git-{VERSION}.tar.xz")),
            "{cause}"
        );
        published.nothing_was_built();
    }

    #[test]
    fn runs_started_together_build_a_release_once() {
        let published = Published::new();
        published.publish_the_tarballs_digest();
        std::thread::scope(|runs| {
            let started: Vec<_> = (0..4)
                .map(|_| runs.spawn(|| published.store().provide(&[VERSION])))
                .collect();
            for run in started {
                run.join().unwrap().unwrap();
            }
        });
        assert_eq!(published.builds(), 1);
    }

    #[test]
    fn a_listed_digest_is_the_one_on_the_line_of_exactly_that_name() {
        let (first, second) = (sha256::hex_digest(b"1"), sha256::hex_digest(b"2"));
        let list = format!(
            "Hash: SHA256\n\n{first}  git-1.0.tar.gz\n{second}  git-1.0.tar.xz\nnot-a-digest  git-2.0.tar.xz\n"
        );
        assert_eq!(
            listed_digest(list.as_bytes(), "git-1.0.tar.xz"),
            Some(second)
        );
        assert_eq!(listed_digest(list.as_bytes(), "git-1.0.tar"), None);
        assert_eq!(listed_digest(list.as_bytes(), "git-2.0.tar.xz"), None);
    }
}
