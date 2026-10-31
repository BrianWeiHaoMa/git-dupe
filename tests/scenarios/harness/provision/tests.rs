//! The checks of obtaining a release, one set on every platform: `Published` stands in for
//! where this platform's procedure obtains a release from.

use super::mingit::windows_tar;
use super::*;
use crate::harness::fresh::FreshDirectory;
use crate::harness::release_site::ReleaseSite;

/// The one release the stand-in publishes. It is not a Git release.
const VERSION: &str = "0.0.0";

/// What a stand-in's archive is pinned to, as the repository's table or digest list has it.
enum Pin {
    TheArchives,
    Another(String),
    /// A digest beside other names on Linux and macOS, and no row on Windows.
    None,
}

/// On Linux and macOS a stand-in for the official source, read through `file://`: one source
/// tarball whose `make install` writes a `bin/git` recording the knobs it was given and adds
/// a line to a tally of builds, its digest list the check's to publish. On Windows a stand-in
/// for Git for Windows's releases, one zip holding `cmd/git.exe` on a `ReleaseSite`, which
/// counts what it is asked, its pin the check's to give.
struct Published {
    root: FreshDirectory,
    site: Option<ReleaseSite>,
    source: String,
    pinned: Option<&'static [MinGit]>,
}

impl Published {
    fn pinned(pin: Pin) -> Published {
        let root = FreshDirectory::create();
        if platform::word() == "windows" {
            Published::zip(root, pin)
        } else {
            Published::tarball(root, pin)
        }
    }

    fn tarball(root: FreshDirectory, pin: Pin) -> Published {
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
        let source = format!("file://{}/source", root.path().display());
        let published = Published {
            root,
            site: None,
            source,
            pinned: None,
        };
        run(Command::new("tar")
            .arg("-cJf")
            .arg(published.archive())
            .arg("-C")
            .arg(published.root.path().join("staged"))
            .arg(format!("git-{VERSION}")))
        .unwrap();
        let digest = published.digest();
        published.publish_list(&match pin {
            Pin::TheArchives => format!("{digest}  git-{VERSION}.tar.xz\n"),
            Pin::Another(other) => format!("{other}  git-{VERSION}.tar.xz\n"),
            // Its digest is there, but only beside other names.
            Pin::None => format!(
                "{digest}  git-{VERSION}.tar.gz\n\
                 {digest}  git-manpages-{VERSION}.tar.xz\n\
                 {digest}  git-{VERSION}.tar.xz.sign\n"
            ),
        });
        published
    }

    fn zip(root: FreshDirectory, pin: Pin) -> Published {
        let row = |sha256: String| MinGit {
            word: VERSION,
            build: VERSION,
            sha256: String::leak(sha256),
        };
        let program = root.path().join("staged/cmd/git.exe");
        fs::create_dir_all(program.parent().unwrap()).unwrap();
        fs::write(&program, "a stand-in").unwrap();
        fs::create_dir(root.path().join("source")).unwrap();
        let published = Published {
            root,
            site: None,
            source: String::new(),
            pinned: None,
        };
        run(Command::new(windows_tar().unwrap())
            .arg("-a")
            .arg("-cf")
            .arg(published.archive())
            .arg("-C")
            .arg(published.root.path().join("staged"))
            .arg("cmd"))
        .unwrap();
        let digest = published.digest();
        let table = match pin {
            Pin::TheArchives => vec![row(digest)],
            Pin::Another(other) => vec![row(other)],
            Pin::None => Vec::new(),
        };
        let archive = (stand_in_asset(), fs::read(published.archive()).unwrap());
        let site = ReleaseSite::serving(&format!("v{VERSION}.windows.1"), vec![archive]);
        Published {
            source: format!("{}/download", site.base()),
            pinned: Some(table.leak()),
            site: Some(site),
            ..published
        }
    }

    fn archive(&self) -> PathBuf {
        let name = if platform::word() == "windows" {
            stand_in_asset()
        } else {
            format!("git-{VERSION}.tar.xz")
        };
        self.root.path().join("source").join(name)
    }

    fn digest(&self) -> String {
        sha256::hex_digest(&fs::read(self.archive()).unwrap())
    }

    fn publish_list(&self, lines: &str) {
        let list = format!(
            "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA256\n\n{lines}\
             -----BEGIN PGP SIGNATURE-----\n\nAAAA\n-----END PGP SIGNATURE-----\n"
        );
        fs::write(self.root.path().join("source").join(DIGEST_LIST), list).unwrap();
    }

    fn store(&self) -> Store {
        Store {
            directory: self.root.path().join("gits"),
            source: self.source.clone(),
            pinned: self.pinned,
        }
    }

    /// What a release of this stand-in holds, and what that file holds.
    fn program(&self) -> (PathBuf, &'static str) {
        let directory = self.store().release_directory(VERSION);
        match self.pinned {
            Some(_) => (directory.join("cmd/git.exe"), "a stand-in"),
            None => (directory.join("bin/git"), "YesPlease 1 1 1 1 1 1 1 1\n"),
        }
    }

    /// How many times the release was built, or its archive asked for.
    fn times_obtained(&self) -> usize {
        if let Some(site) = &self.site {
            return site.asked().len();
        }
        match fs::read_to_string(self.root.path().join("tally")) {
            Ok(tally) => tally.lines().count(),
            Err(cause) if cause.kind() == ErrorKind::NotFound => 0,
            Err(cause) => panic!("the tally of builds: {cause}"),
        }
    }

    fn remove_the_source(&mut self) {
        self.site = None;
        fs::remove_dir_all(self.root.path().join("source")).unwrap();
    }

    fn nothing_was_obtained(&self) {
        let store = self.store();
        assert!(!store.release_directory(VERSION).exists());
        assert!(!store.directory.join("work").exists());
        // A bad archive is asked for once; a build it never reached is the tally's.
        if self.pinned.is_none() {
            assert_eq!(self.times_obtained(), 0);
        }
    }
}

fn stand_in_asset() -> String {
    format!("MinGit-{VERSION}-64-bit.zip")
}

#[test]
fn a_release_is_obtained_once_and_then_left_alone() {
    let mut published = Published::pinned(Pin::TheArchives);
    let store = published.store();
    assert_eq!(store.absent(&[VERSION]), [VERSION]);
    store.provide(&[VERSION]).unwrap();

    let (program, held) = published.program();
    let marker = store.release_directory(VERSION).join(MARKER);
    assert_eq!(fs::read_to_string(&program).unwrap(), held);
    assert_eq!(
        fs::read_to_string(&marker).unwrap(),
        format!("{}\n", published.digest())
    );
    assert!(store.absent(&[VERSION]).is_empty());
    assert!(!store.directory.join("work").exists());
    assert_eq!(published.times_obtained(), 1);

    // With its source gone, a present release still provides: nothing is downloaded.
    let modified = |path: &Path| fs::metadata(path).unwrap().modified().unwrap();
    let before = [modified(&program), modified(&marker)];
    published.remove_the_source();
    store.provide(&[VERSION]).unwrap();
    assert_eq!([modified(&program), modified(&marker)], before);
}

#[test]
fn a_directory_without_its_marker_is_obtained_again_not_used() {
    let published = Published::pinned(Pin::TheArchives);
    let store = published.store();
    store.provide(&[VERSION]).unwrap();

    let directory = store.release_directory(VERSION);
    fs::remove_file(directory.join(MARKER)).unwrap();
    fs::write(directory.join("half-installed"), "").unwrap();
    assert_eq!(store.absent(&[VERSION]), [VERSION]);
    store.provide(&[VERSION]).unwrap();

    assert!(directory.join(MARKER).is_file());
    assert!(!directory.join("half-installed").exists());
    assert_eq!(published.times_obtained(), 2);
}

#[test]
fn an_archive_that_does_not_hash_to_its_digest_obtains_nothing() {
    let other = sha256::hex_digest(b"another archive");
    let published = Published::pinned(Pin::Another(other.clone()));

    let cause = published.store().provide(&[VERSION]).unwrap_err();
    assert!(cause.contains(&other), "{cause}");
    assert!(cause.contains(&published.digest()), "{cause}");
    published.nothing_was_obtained();
}

#[test]
fn a_release_with_no_digest_of_its_own_obtains_nothing() {
    let published = Published::pinned(Pin::None);

    let cause = published.store().provide(&[VERSION]).unwrap_err();
    let named = match published.pinned {
        Some(_) => format!("no MinGit archive is pinned for {VERSION}"),
        None => format!("no SHA-256 entry for git-{VERSION}.tar.xz"),
    };
    assert!(cause.contains(&named), "{cause}");
    published.nothing_was_obtained();
    assert_eq!(published.times_obtained(), 0);
}

#[test]
fn runs_started_together_obtain_a_release_once() {
    let published = Published::pinned(Pin::TheArchives);
    std::thread::scope(|runs| {
        let started: Vec<_> = (0..4)
            .map(|_| runs.spawn(|| published.store().provide(&[VERSION])))
            .collect();
        for run in started {
            run.join().unwrap().unwrap();
        }
    });
    assert_eq!(published.times_obtained(), 1);
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
