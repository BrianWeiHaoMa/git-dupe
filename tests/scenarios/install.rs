//! `install.sh`, run as the line a reader pastes runs it: `sh` reading the script on its
//! standard input, from a directory of its own, with the scenario's `HOME`, a `TMPDIR` of
//! its own, and `GIT_DUPE_RELEASES_URL` naming a stand-in for GitHub's releases pages on
//! the loopback interface (`harness/release_site.rs`), so that nothing here asks another
//! machine. The stand-in's latest release holds the archive this check command made and
//! a `.sha256` written as `shasum -a 256` writes one. After a run that succeeds, Git finds
//! git-dupe and `man` finds its page in the two directories `INSTALL` names, and nowhere
//! else (P2, P3, S19, G24); a run that fails before the copies installs nothing, and a
//! machine no archive is built for is refused before anything is asked (F1, N10).
//! What the documents show of it is pinned in `install_lines`; on Windows, where this file
//! reports itself unavailable, `install_powershell` runs `install.ps1` the same way.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, MACHINES_WITHOUT_AN_ARCHIVE, Output, ReleaseSite, Scenario, asking_for_the_page,
    entry_names, every_text_shown, hex_digest, identity, install_note, lines_of, mode_is,
    path_for_an_install, release_archive, report, symlink, text, under_each_release, word,
    write_executable,
};

/// The line the installed executable prints for `git dupe --version`.
const VERSION_LINE: &str = concat!("git-dupe version ", env!("CARGO_PKG_VERSION"), "\n");

/// The `sh` of the pasted line with a umask that keeps every bit from the group and the
/// others, reading the script on its standard input as the plain `sh` does.
const UNDER_UMASK_077: [&str; 2] = ["-c", "umask 077 && exec sh"];

/// Whether this platform has the POSIX `sh` and the tools `install.sh` names. Where it has
/// not, `check` reports itself, before any archive is made for it (P3).
fn the_script_runs_here(check: &str) -> bool {
    let here = word() != "windows";
    if !here {
        report(&format!("install: {check} unavailable: install.sh's run"));
    }
    here
}

/// The tag of the release the stand-in serves, as a release is tagged.
fn tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// The name of this machine's archive, as a release names it and the archive this check
/// command made is named (P3).
fn archive_name() -> String {
    let made = release_archive();
    text(Path::new(made.path.file_name().unwrap())).to_owned()
}

/// An archive and its `.sha256`, which names it by its bare name, as a release publishes
/// them.
fn published(archive: Vec<u8>) -> Vec<(String, Vec<u8>)> {
    let name = archive_name();
    let digest = format!("{}  {name}\n", hex_digest(&archive));
    vec![
        (format!("{name}.sha256"), digest.into_bytes()),
        (name, archive),
    ]
}

/// A stand-in whose latest release holds the archive this check command made.
fn site_of_the_release() -> ReleaseSite {
    let archive = fs::read(&release_archive().path).unwrap();
    ReleaseSite::serving(&tag(), published(archive))
}

/// The paths a run asked the stand-in for, in order, whatever the method.
fn paths_asked(site: &ReleaseSite) -> Vec<String> {
    site.asked()
        .iter()
        .map(|request| request.split(' ').nth(1).unwrap_or("").to_owned())
        .collect()
}

/// Every path below `directory`, relative to it, directories included, in order.
fn everything_below(directory: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for name in entry_names(directory) {
        let path = directory.join(&name);
        let name = text(Path::new(&name)).to_owned();
        found.push(name.clone());
        if fs::symlink_metadata(&path).unwrap().is_dir() {
            found.extend(
                everything_below(&path)
                    .into_iter()
                    .map(|below| format!("{name}/{below}")),
            );
        }
    }
    found
}

/// The script run as the pasted line runs it, in one scenario.
struct Install<'s> {
    s: &'s Scenario,
    /// The scenario's `HOME`.
    home: PathBuf,
    /// The run's `TMPDIR`, empty before every run.
    temporary: PathBuf,
    /// The directory the run starts from, outside the repository and the home.
    elsewhere: PathBuf,
}

impl<'s> Install<'s> {
    fn of(s: &'s Scenario) -> Install<'s> {
        let install = Install {
            s,
            home: s.dir().join("home"),
            temporary: s.dir().join("temporary"),
            elsewhere: s.dir().join("elsewhere"),
        };
        fs::create_dir(&install.temporary).unwrap();
        fs::create_dir(&install.elsewhere).unwrap();
        install
    }

    fn bin(&self) -> PathBuf {
        self.home.join(".local/bin")
    }

    fn manual_path(&self) -> PathBuf {
        self.home.join(".local/share/man")
    }

    /// `script` on the standard input of the `sh` that `shell` starts, with `path` its
    /// `PATH`, against `site`.
    fn run(&self, site: &ReleaseSite, path: &OsStr, shell: &[&str], script: &[u8]) -> Output {
        self.s
            .program("sh", shell)
            .input(script)
            .from(&self.elsewhere)
            .variable("PATH", path)
            .variable("TMPDIR", &self.temporary)
            .variable("GIT_DUPE_RELEASES_URL", site.base())
            .run()
    }

    /// The script as the repository holds it, run as `run` runs it.
    fn pasted(&self, site: &ReleaseSite, path: &OsStr, shell: &[&str]) -> Output {
        let script = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh")).unwrap();
        self.run(site, path, shell, &script)
    }

    /// After a run that installed: exactly the release's executable, mode 755, and its
    /// page, mode 644, under `~/.local`, in the directories `INSTALL` names, and nothing
    /// left in the temporary directory.
    fn installed(&self, output: &Output) {
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(
            everything_below(&self.home.join(".local")),
            [
                "bin",
                "bin/git-dupe",
                "share",
                "share/man",
                "share/man/man1",
                "share/man/man1/git-dupe.1"
            ],
            "{output:?}"
        );
        let made = release_archive();
        let (executable, page) = (
            self.bin().join("git-dupe"),
            self.manual_path().join("man1/git-dupe.1"),
        );
        assert!(
            fs::read(&executable).unwrap() == fs::read(&made.executable).unwrap(),
            "{} is not the release executable",
            executable.display()
        );
        assert_eq!(fs::read(&page).unwrap(), fs::read(&made.page).unwrap());
        mode_is(self.s, &executable, 0o755, "install");
        mode_is(self.s, &page, 0o644, "install");
        assert!(entry_names(&self.temporary).is_empty(), "{output:?}");
    }

    /// After a run that failed: nothing under `~/.local`, which no run made, and nothing
    /// left in the temporary directory.
    fn nothing_installed(&self, output: &Output) {
        assert_ne!(output.end, End::Code(0), "{output:?}");
        assert!(
            fs::symlink_metadata(self.home.join(".local")).is_err(),
            "{output:?}"
        );
        assert!(entry_names(&self.temporary).is_empty(), "{output:?}");
    }
}

#[test]
fn the_pasted_line_installs_the_latest_release_s_executable_and_page() {
    if !the_script_runs_here("the pasted line") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let output = install.pasted(&site, &path_for_an_install(s, &[]), &[]);
        install.installed(&output);
        let name = archive_name();
        let base = format!("/releases/download/{}", tag());
        assert_eq!(
            paths_asked(&site),
            [
                "/releases/latest".to_owned(),
                format!("/releases/tag/{}", tag()),
                format!("{base}/{name}"),
                format!("{base}/{name}.sha256"),
            ]
        );
        // Where the files went, the version the executable prints, and, since
        // `~/.local/bin` is not on the run's `PATH`, how to put it there; the release's
        // Git is a supported one, so no warning.
        let said = String::from_utf8_lossy(&output.stdout);
        let executable = install.bin().join("git-dupe");
        for named in [
            text(&executable),
            VERSION_LINE,
            "~/.local/bin",
            "export PATH=",
        ] {
            assert!(said.contains(named), "{named:?} is not in: {output:?}");
        }
        assert!(output.stderr.is_empty(), "{output:?}");

        // With that directory on `PATH`, Git finds git-dupe and `man` finds its page.
        let path = path_for_an_install(s, &[&install.bin()]);
        let version = s.git(["dupe", "--version"]).variable("PATH", &path).run();
        assert_eq!(version.end, End::Code(0), "{version:?}");
        assert_eq!(version.stdout, VERSION_LINE.as_bytes(), "{version:?}");
        let [man, help] = asking_for_the_page(s, &install.manual_path())
            .map(|asked| asked.variable("PATH", &path).run());
        assert_eq!(man.end, End::Code(0), "{man:?}");
        every_text_shown(&lines_of(&man.stdout), "`man git-dupe` after install.sh");
        assert_eq!(help.end, End::Code(0), "{help:?}");
        assert_eq!(help.stdout, man.stdout, "`git dupe --help`: {help:?}");
    });
}

/// With `~/.local/bin` already on `PATH` and a git-dupe there, as on a machine where an
/// earlier release was installed: the release replaces it, and the script does not tell
/// the user to add what they have. Each file is replaced by a rename onto its path, so
/// the executable is a new file and a page that was a symbolic link is replaced as a
/// link, its target untouched, where a copy onto the path would have written through it.
#[test]
fn run_again_it_replaces_the_installed_files_and_advises_nothing() {
    if !the_script_runs_here("a second run") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let executable = install.bin().join("git-dupe");
        write_executable(s, &executable, b"#!/bin/sh\necho an earlier git-dupe\n");
        let earlier = identity(&executable);
        let elsewhere = s.dir().join("an-earlier-page.1");
        fs::write(&elsewhere, b"an earlier page\n").unwrap();
        let man1 = install.manual_path().join("man1");
        fs::create_dir_all(&man1).unwrap();
        symlink(&elsewhere, man1.join("git-dupe.1")).unwrap();

        let output = install.pasted(&site, &path_for_an_install(s, &[&install.bin()]), &[]);
        install.installed(&output);
        assert_ne!(identity(&executable), earlier, "{output:?}");
        assert!(
            fs::symlink_metadata(man1.join("git-dupe.1"))
                .unwrap()
                .is_file()
        );
        assert_eq!(fs::read(&elsewhere).unwrap(), b"an earlier page\n");
        let said = String::from_utf8_lossy(&output.stdout);
        assert!(said.contains(VERSION_LINE), "{output:?}");
        assert!(!said.contains("PATH"), "{output:?}");
    });
}

/// The copies have the archive's modes, not the ones a strict umask gives a copy.
#[test]
fn under_a_strict_umask_the_files_have_the_archive_s_modes() {
    if !the_script_runs_here("a strict umask") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let output = install.pasted(&site, &path_for_an_install(s, &[]), &UNDER_UMASK_077);
        install.installed(&output);
    });
}

/// A `.sha256` that is not the archive's own digest and bare name: one hex digit off, or
/// the digest of another file by that file's name, which `shasum -c` would check in the
/// archive's place, or the archive's digest by another archive's name.
#[test]
fn an_archive_that_does_not_match_its_digest_installs_nothing() {
    if !the_script_runs_here("a mismatched digest") {
        return;
    }
    let archive = fs::read(&release_archive().path).unwrap();
    let [(sidecar, right), _] = &published(archive.clone())[..] else {
        panic!("an archive and its .sha256");
    };
    let mut off_by_a_digit = right.clone();
    off_by_a_digit[0] = if right[0] == b'0' { b'1' } else { b'0' };
    let wrong = [
        off_by_a_digit,
        format!("{}  /dev/null\n", hex_digest(b"")).into_bytes(),
        format!("{}  git-dupe-other.tar\n", hex_digest(&archive)).into_bytes(),
    ];
    under_each_release(|s| {
        let install = Install::of(s);
        for published in &wrong {
            let files = vec![
                (sidecar.clone(), published.clone()),
                (archive_name(), archive.clone()),
            ];
            let site = ReleaseSite::serving(&tag(), files);
            let output = install.pasted(&site, &path_for_an_install(s, &[]), &[]);
            install.nothing_installed(&output);
            let said = String::from_utf8_lossy(&output.stderr);
            assert!(said.contains("SHA-256 digest"), "{output:?}");
        }
    });
}

/// An archive of the release's shape whose executable exits 1, as one linked against a
/// newer C library, or built for another machine, fails where it is run.
#[test]
fn an_executable_that_does_not_run_here_installs_nothing() {
    if !the_script_runs_here("an executable that does not run") {
        return;
    }
    let made = release_archive();
    under_each_release(|s| {
        let leading = format!("git-dupe-{}", env!("CARGO_PKG_VERSION"));
        let staging = s.dir().join("staging");
        write_executable(
            s,
            &staging.join(&leading).join("git-dupe"),
            b"#!/bin/sh\nexit 1\n",
        );
        fs::copy(&made.page, staging.join(&leading).join("git-dupe.1")).unwrap();
        fs::copy(install_note(), staging.join(&leading).join("INSTALL")).unwrap();
        let archive = s.dir().join(archive_name());
        let mut words = ["-cf", text(&archive), "-C", text(&staging)]
            .map(str::to_owned)
            .to_vec();
        words.extend(["git-dupe", "git-dupe.1", "INSTALL"].map(|name| format!("{leading}/{name}")));
        s.program("tar", words).succeeds();
        let site = ReleaseSite::serving(&tag(), published(fs::read(&archive).unwrap()));
        let install = Install::of(s);
        let output = install.pasted(&site, &path_for_an_install(s, &[]), &[]);
        install.nothing_installed(&output);
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(
            said.contains("from source") && said.contains("docs/install.md#from-source"),
            "{output:?}"
        );
    });
}

/// A machine no archive is built for, as `uname` names it, is refused before the script
/// asks anything of the releases pages (F1, N10).
#[test]
fn a_machine_no_archive_is_built_for_is_refused_before_anything_is_asked() {
    if !the_script_runs_here("a machine with no archive") {
        return;
    }
    under_each_release(|s| {
        let install = Install::of(s);
        for (system, machine) in MACHINES_WITHOUT_AN_ARCHIVE {
            let stand_ins = s.dir().join(format!("{system}-{machine}"));
            let uname = format!(
                "#!/bin/sh\ncase $1 in\n-s) echo {system} ;;\n-m) echo {machine} ;;\n*) exit 1 ;;\nesac\n"
            );
            write_executable(s, &stand_ins.join("uname"), uname.as_bytes());
            let site = site_of_the_release();
            let output = install.pasted(&site, &path_for_an_install(s, &[&stand_ins]), &[]);
            install.nothing_installed(&output);
            assert_eq!(site.asked(), Vec::<String>::new(), "{output:?}");
            let said = String::from_utf8_lossy(&output.stderr);
            for named in [
                system,
                machine,
                "from source",
                "docs/install.md#from-source",
            ] {
                assert!(said.contains(named), "{named:?} is not in: {output:?}");
            }
        }
    });
}

#[test]
fn with_no_release_to_find_nothing_is_installed() {
    if !the_script_runs_here("no release to find") {
        return;
    }
    under_each_release(|s| {
        let site = ReleaseSite::without_a_release();
        let install = Install::of(s);
        let output = install.pasted(&site, &path_for_an_install(s, &[]), &[]);
        install.nothing_installed(&output);
        assert_eq!(paths_asked(&site), ["/releases/latest"]);
    });
}

/// The script cut short at the end of any of its lines, as a download that stopped there
/// leaves it, defines what it read and runs nothing.
#[test]
fn a_script_cut_short_runs_nothing() {
    if !the_script_runs_here("a script cut short") {
        return;
    }
    let script = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh")).unwrap();
    let ends: Vec<usize> = script
        .iter()
        .enumerate()
        .filter(|&(_, &byte)| byte == b'\n')
        .map(|(at, _)| at + 1)
        .filter(|&end| end < script.len())
        .collect();
    assert!(ends.len() > 1, "install.sh has more than one line");
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let path = path_for_an_install(s, &[]);
        for &end in &ends {
            let output = install.run(&site, &path, &[], &script[..end]);
            assert!(
                fs::symlink_metadata(install.home.join(".local")).is_err()
                    && site.asked().is_empty()
                    && entry_names(&install.temporary).is_empty(),
                "install.sh cut after its byte {end}: {output:?}"
            );
        }
    });
}

/// The install goes on whatever Git is on `PATH`, and names one older than the oldest
/// supported release in a warning.
#[test]
fn an_older_git_on_path_is_named_in_a_warning_and_the_install_goes_on() {
    if !the_script_runs_here("an older Git on PATH") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let older = s.dir().join("older-git");
        write_executable(
            s,
            &older.join("git"),
            b"#!/bin/sh\necho 'git version 2.39.5'\n",
        );
        let output = install.pasted(&site, &path_for_an_install(s, &[&older]), &[]);
        install.installed(&output);
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(
            said.contains("warning") && said.contains("2.39.5") && said.contains("2.43.0"),
            "{output:?}"
        );
    });
}
