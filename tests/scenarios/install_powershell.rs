//! `install.ps1`, run as the line a reader pastes runs it: Windows PowerShell 5.1 under the
//! strictest execution policy, `irm <url> | iex`, from a directory of its own, with the
//! scenario's home as `USERPROFILE` and `HOME`, a `TEMP` and `TMP` of its own, and
//! `GIT_DUPE_RELEASES_URL` naming a stand-in on the loopback interface that serves the script
//! beside the releases pages (`harness/release_site.rs`), so that nothing here asks another
//! machine (L4, P3, S25). Every run compares every value of `HKCU\Environment` before and
//! after it, because a script that helpfully sets the user's `Path` installs just as well.
//! Each check reports itself unavailable off Windows, before any archive is asked for.
//!
//! Of `install`'s cases this file leaves out: the strict umask, because Windows has no modes;
//! the machine no archive is built for, because the architecture comes from .NET, not from a
//! program a check can stand in for; the executable that does not run, because no executable
//! Windows gives is known to fail `--version` and the harness compiles none for it; and the
//! older Git on `PATH`, because a stand-in `git` there must be an executable, and no Git on
//! `PATH` covers the warning. Its small fixtures are written again here (`published`,
//! `paths_asked`, `everything_below`) rather than moved out of `install`.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::harness::{
    End, Output, ReleaseSite, Scenario, entry_names, hex_digest, identity, path_for_an_install,
    release_archive, report, sleeping, text, under_each_release, windows_powershell, word,
};

/// The line the installed executable prints for `git dupe --version`.
const VERSION_LINE: &str = concat!("git-dupe version ", env!("CARGO_PKG_VERSION"), "\n");

/// Whether this platform has the shell `install.ps1` is pasted into. Where it has not, `check`
/// reports itself, before any archive is made for it (P3).
fn the_script_runs_here(check: &str) -> bool {
    let here = word() == "windows";
    if !here {
        report(&format!(
            "install_powershell: {check} unavailable: install.ps1's run"
        ));
    }
    here
}

/// The tag of the release the stand-in serves.
fn tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn archive_name() -> String {
    let made = release_archive();
    text(Path::new(made.path.file_name().unwrap())).to_owned()
}

/// The script as the repository holds it.
fn script() -> Vec<u8> {
    fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("install.ps1")).unwrap()
}

/// The zip this check command made and `sidecar` as its `.sha256`, as a release publishes them.
fn published(sidecar: String) -> Vec<(String, Vec<u8>)> {
    let archive = fs::read(&release_archive().path).unwrap();
    let name = archive_name();
    vec![
        (format!("{name}.sha256"), sidecar.into_bytes()),
        (name, archive),
    ]
}

/// A stand-in whose latest release is this check command's, under the `.sha256` it wrote.
fn site_of_the_release() -> ReleaseSite {
    let sidecar = fs::read_to_string(&release_archive().digest).unwrap();
    ReleaseSite::serving(&tag(), published(sidecar)).with_script(script())
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

/// Every value of the user's environment in the registry, as `reg.exe` prints them.
fn user_settings(s: &Scenario) -> Vec<u8> {
    let root = std::env::var_os("SystemRoot").expect("SystemRoot names Windows's directory");
    let reg = Path::new(&root).join(r"System32\reg.exe");
    s.program(reg, [r"query", r"HKCU\Environment"])
        .succeeds()
        .stdout
}

/// What standard output and standard error hold, as text.
fn said(printed: &[u8]) -> String {
    String::from_utf8_lossy(printed).into_owned()
}

/// The script run as the pasted line runs it, in one scenario.
struct Install<'s> {
    s: &'s Scenario,
    /// The scenario's home, the run's `USERPROFILE` and `HOME`.
    home: PathBuf,
    /// The run's `TEMP` and `TMP`, empty before every run.
    temporary: PathBuf,
    /// The directory the run starts from, outside the repository and the home.
    elsewhere: PathBuf,
}

impl<'s> Install<'s> {
    fn of(s: &'s Scenario) -> Install<'s> {
        Install::under(s, s.dir())
    }

    fn under(s: &'s Scenario, directory: &Path) -> Install<'s> {
        let install = Install {
            s,
            home: directory.join("home"),
            temporary: directory.join("temporary"),
            elsewhere: directory.join("elsewhere"),
        };
        for made in [&install.home, &install.temporary, &install.elsewhere] {
            fs::create_dir_all(made).unwrap();
        }
        install
    }

    fn bin(&self) -> PathBuf {
        self.home.join(r".local\bin")
    }

    fn executable(&self) -> PathBuf {
        self.bin().join("git-dupe.exe")
    }

    /// `command` in PowerShell, against `site`, with `path` its `PATH`.
    fn powershell(&self, site: &ReleaseSite, path: &OsStr, command: &str) -> Output {
        let words = [
            "-NoProfile",
            "-ExecutionPolicy",
            "Restricted",
            "-Command",
            command,
        ];
        self.s
            .program(windows_powershell(), words)
            .from(&self.elsewhere)
            .variable("PATH", path)
            .variable("USERPROFILE", &self.home)
            .variable("HOME", &self.home)
            .variable("TEMP", &self.temporary)
            .variable("TMP", &self.temporary)
            .variable("GIT_DUPE_RELEASES_URL", site.base())
            .run()
    }

    /// The line a reader pastes, with the stand-in's address in the place of GitHub's.
    fn line(site: &ReleaseSite) -> String {
        format!("irm {} | iex", site.script_url())
    }

    /// `command` as `powershell` runs it, the user's settings compared around it.
    fn watched(&self, site: &ReleaseSite, path: &OsStr, command: &str) -> Output {
        let before = user_settings(self.s);
        let output = self.powershell(site, path, command);
        assert_eq!(
            said(&user_settings(self.s)),
            said(&before),
            "HKCU\\Environment after {command}: {output:?}"
        );
        output
    }

    /// The pasted line, its settings compared around it.
    fn pasted(&self, site: &ReleaseSite, path: &OsStr) -> Output {
        self.watched(site, path, &Install::line(site))
    }

    /// After a run that installed: exactly the release's executable under `~\.local`, nothing
    /// left in the temporary directory, the version line printed.
    fn installed(&self, output: &Output) {
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(
            everything_below(&self.home.join(".local")),
            ["bin", "bin/git-dupe.exe"],
            "{output:?}"
        );
        self.holds_the_release(output);
    }

    /// `git-dupe.exe` is the release's, the temporary directory empty, the version line and the
    /// installed path printed.
    fn holds_the_release(&self, output: &Output) {
        let (installed, made) = (self.executable(), release_archive());
        assert!(
            fs::read(&installed).unwrap() == fs::read(&made.executable).unwrap(),
            "the installed executable is not the release's: {output:?}"
        );
        assert!(entry_names(&self.temporary).is_empty(), "{output:?}");
        let printed = said(&output.stdout);
        assert!(printed.contains(text(&installed)), "{output:?}");
        assert!(printed.contains(VERSION_LINE.trim_end()), "{output:?}");
    }

    /// After a run that refused: exit 1, one `install.ps1:` line, nothing installed, nothing left.
    fn refused(&self, output: &Output, saying: &str) {
        assert_eq!(output.end, End::Code(1), "{output:?}");
        let complaint = said(&output.stderr);
        let lines: Vec<&str> = complaint.lines().collect();
        assert!(
            lines.len() == 1 && lines[0].starts_with("install.ps1: "),
            "{output:?}"
        );
        assert!(complaint.contains(saying), "{output:?}");
        assert!(entry_names(&self.temporary).is_empty(), "{output:?}");
    }

    fn nothing_installed(&self, output: &Output) {
        assert!(
            fs::symlink_metadata(self.home.join(".local")).is_err(),
            "{output:?}"
        );
    }
}

/// P3's shape, read on every platform: a cut prefix can run only what stands outside a
/// function, so that nothing may, not even a preference, and the last line alone calls.
#[test]
fn every_command_of_the_script_stands_in_a_function_its_last_line_calls() {
    let whole = script();
    assert!(whole.is_ascii() && !whole.contains(&b'\r'), "ASCII, LF");
    let whole = String::from_utf8(whole).unwrap();
    let outside: Vec<&str> = whole
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with([' ', '#', '}']))
        .filter(|line| !line.starts_with("function "))
        .collect();
    assert_eq!(outside, [whole.lines().next_back().unwrap()]);
    assert_eq!(whole.matches("\nInstall-GitDupe\n").count(), 1);
    assert!(whole.ends_with("\nInstall-GitDupe\n"));
}

/// With nothing installed and `~\.local\bin` not on `PATH`: the release's executable there, its
/// `PATH` line printed and run for nobody, and `git dupe --version` answering once the reader
/// has put it on `PATH` (L4, P3, G24).
#[test]
fn the_pasted_line_installs_the_latest_release_s_executable() {
    if !the_script_runs_here("the pasted line") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let output = install.pasted(&site, &path_for_an_install(s, &[]));
        install.installed(&output);
        assert!(output.stderr.is_empty(), "{output:?}");
        let printed = said(&output.stdout);
        for advice in [r".local\bin", "$env:Path =", "SetEnvironmentVariable"] {
            assert!(printed.contains(advice), "{advice}: {output:?}");
        }
        let (zip, tag) = (archive_name(), tag());
        assert_eq!(
            paths_asked(&site),
            [
                "/install.ps1".to_owned(),
                "/releases/latest".to_owned(),
                format!("/releases/download/{tag}/{zip}"),
                format!("/releases/download/{tag}/{zip}.sha256"),
            ]
        );
        let path = path_for_an_install(s, &[&install.bin()]);
        let answer = s
            .git(["dupe", "--version"])
            .variable("PATH", &path)
            .succeeds();
        assert_eq!(answer.stdout, VERSION_LINE.as_bytes(), "{answer:?}");
    });
}

/// The form Git Bash's `sha256sum` writes, which marks the file as binary.
#[test]
fn a_digest_file_in_the_binary_form_installs_the_same() {
    if !the_script_runs_here("a digest file in the binary form") {
        return;
    }
    under_each_release(|s| {
        let archive = fs::read(&release_archive().path).unwrap();
        let sidecar = format!("{} *{}\n", hex_digest(&archive), archive_name());
        let site = ReleaseSite::serving(&tag(), published(sidecar)).with_script(script());
        let install = Install::of(s);
        let output = install.pasted(&site, &path_for_an_install(s, &[]));
        install.installed(&output);
    });
}

/// A digest one digit off, the digest of nothing under another file's name, and the zip's own
/// digest under another name: a `.sha256` is believed only for the zip it names, and only
/// where it is that zip's (P3).
#[test]
fn a_digest_file_that_is_not_the_zip_s_installs_nothing() {
    if !the_script_runs_here("a digest file that is not the zip's") {
        return;
    }
    under_each_release(|s| {
        let archive = fs::read(&release_archive().path).unwrap();
        let (digest, name) = (hex_digest(&archive), archive_name());
        let last = if digest.ends_with('0') { '1' } else { '0' };
        let one_digit_off = format!("{}{last}", &digest[..63]);
        for (at, sidecar) in [
            format!("{one_digit_off}  {name}\n"),
            format!("{}  NUL\n", hex_digest(b"")),
            format!("{digest}  git-dupe-other.zip\n"),
        ]
        .into_iter()
        .enumerate()
        {
            let site = ReleaseSite::serving(&tag(), published(sidecar)).with_script(script());
            let install = Install::under(s, &s.dir().join(at.to_string()));
            let output = install.pasted(&site, &path_for_an_install(s, &[]));
            install.refused(&output, "SHA-256 digest");
            install.nothing_installed(&output);
        }
    });
}

#[test]
fn no_release_to_find_installs_nothing() {
    if !the_script_runs_here("no release to find") {
        return;
    }
    under_each_release(|s| {
        let site = ReleaseSite::without_a_release().with_script(script());
        let install = Install::of(s);
        let output = install.pasted(&site, &path_for_an_install(s, &[]));
        install.refused(&output, "latest release");
        install.nothing_installed(&output);
        assert_eq!(paths_asked(&site), ["/install.ps1", "/releases/latest"]);
    });
}

/// How many cuts run at once: a PowerShell starts in a fraction of a second, and the script
/// has a few hundred lines to cut after, under every listed release.
const CUTS_AT_ONCE: usize = 8;

/// The script cut short at the end of any of its lines, as a download that stopped there
/// leaves it: inside a function a parse error, between functions a definition, and in neither
/// case does a line of it run (P3, S25).
#[test]
fn a_script_cut_short_runs_nothing() {
    if !the_script_runs_here("a script cut short") {
        return;
    }
    let whole = script();
    let ends: Vec<usize> = whole
        .iter()
        .enumerate()
        .filter(|&(_, &byte)| byte == b'\n')
        .map(|(at, _)| at + 1)
        .filter(|&end| end < whole.len())
        .collect();
    assert!(ends.len() > 1, "install.ps1 has more than one line");
    under_each_release(|s| {
        let path = path_for_an_install(s, &[]);
        let before = user_settings(s);
        std::thread::scope(|threads| {
            for (at, these) in ends.chunks(ends.len().div_ceil(CUTS_AT_ONCE)).enumerate() {
                let (path, whole) = (&path, &whole);
                threads.spawn(move || {
                    let install = Install::under(s, &s.dir().join(at.to_string()));
                    for &end in these {
                        let site = ReleaseSite::serving(&tag(), Vec::new())
                            .with_script(whole[..end].to_vec());
                        let output = install.powershell(&site, path, &Install::line(&site));
                        assert!(
                            fs::symlink_metadata(install.home.join(".local")).is_err()
                                && paths_asked(&site) == ["/install.ps1"]
                                && entry_names(&install.temporary).is_empty(),
                            "install.ps1 cut after its byte {end}: {output:?}"
                        );
                    }
                });
            }
        });
        assert_eq!(said(&user_settings(s)), said(&before));
    });
}

/// With `~\.local\bin` on `PATH` and an earlier file there: replaced, nothing kept aside, and no
/// advice about a `PATH` that already holds it.
#[test]
fn run_again_it_replaces_the_installed_executable_and_advises_nothing() {
    if !the_script_runs_here("run again") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        fs::create_dir_all(install.bin()).unwrap();
        fs::write(install.executable(), b"an earlier executable").unwrap();
        let output = install.pasted(&site, &path_for_an_install(s, &[&install.bin()]));
        install.installed(&output);
        assert!(!said(&output.stdout).contains("$env:Path"), "{output:?}");
    });
}

/// A process kept from outliving its check.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A running `git-dupe.exe` cannot be replaced and can be renamed away (S25): the new one takes
/// its name and the old one stands aside, said on standard output. A second run, while that
/// one still runs, cannot clear the way and refuses, changing nothing; a third, once it has
/// ended, removes it.
#[test]
fn a_running_executable_goes_aside_and_a_later_run_removes_it() {
    if !the_script_runs_here("a running executable") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let path = path_for_an_install(s, &[&install.bin()]);
        let target = install.executable();
        let aside = install.bin().join("git-dupe.exe.old");
        fs::create_dir_all(install.bin()).unwrap();

        let sleeper = sleeping(60);
        let earlier = fs::read(sleeper.get_program()).unwrap();
        fs::write(&target, &earlier).unwrap();
        let running = Command::new(&target)
            .args(sleeper.get_args())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let running = Running(running);

        let output = install.pasted(&site, &path);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        install.holds_the_release(&output);
        assert!(said(&output.stdout).contains(text(&aside)), "{output:?}");
        assert!(fs::read(&aside).unwrap() == earlier, "{output:?}");

        let identities = [identity(&target), identity(&aside)];
        let refused = install.pasted(&site, &path);
        install.refused(&refused, text(&aside));
        assert_eq!(
            [identity(&target), identity(&aside)],
            identities,
            "{refused:?}"
        );
        assert_eq!(
            everything_below(&install.bin()),
            ["git-dupe.exe", "git-dupe.exe.old"],
            "{refused:?}"
        );

        drop(running);
        let output = install.pasted(&site, &path);
        install.installed(&output);
    });
}

/// A run killed between its two renames leaves the old file aside and nothing at the path:
/// the next run completes the install, and that file is gone only once the new one stands.
#[test]
fn a_run_killed_between_the_renames_is_completed_by_the_next() {
    if !the_script_runs_here("a run killed between the renames") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        fs::create_dir_all(install.bin()).unwrap();
        fs::write(install.bin().join("git-dupe.exe.old"), b"the old file").unwrap();
        let output = install.pasted(&site, &path_for_an_install(s, &[]));
        install.installed(&output);
    });
}

#[test]
fn a_directory_at_the_executable_s_path_is_refused() {
    if !the_script_runs_here("a directory in the way") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let in_the_way = install.executable();
        fs::create_dir_all(&in_the_way).unwrap();
        fs::write(in_the_way.join("held"), b"held").unwrap();
        let output = install.pasted(&site, &path_for_an_install(s, &[]));
        install.refused(&output, text(&in_the_way));
        assert_eq!(
            everything_below(&install.bin()),
            ["git-dupe.exe", "git-dupe.exe/held"],
            "{output:?}"
        );
    });
}

/// The install goes on with no Git on `PATH`, and says what git-dupe needs.
#[test]
fn no_git_on_path_is_named_in_a_warning_and_the_install_goes_on() {
    if !the_script_runs_here("no git on PATH") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let path = install.home.join(".local");
        let output = install.pasted(&site, path.as_os_str());
        install.installed(&output);
        let complaint = said(&output.stderr);
        assert!(
            complaint.contains("warning") && complaint.contains("2.43.0"),
            "{output:?}"
        );
    });
}

/// A success returns: what follows the pasted line in its session runs, and finds `$env:Path`
/// and the last exit code as they were. A script that sets the session's `Path`, or ends with
/// `exit 0`, passes every other check here and fails this one.
#[test]
fn the_session_goes_on_as_it_was() {
    if !the_script_runs_here("the session") {
        return;
    }
    under_each_release(|s| {
        let site = site_of_the_release();
        let install = Install::of(s);
        let line = format!(
            "$LASTEXITCODE = 7; $before = $env:Path; {}; \
             if ($env:Path -cne $before -or $LASTEXITCODE -ne 7) {{ exit 99 }}; \
             [Console]::Out.WriteLine('the session went on')",
            Install::line(&site)
        );
        let output = install.watched(&site, &path_for_an_install(s, &[]), &line);
        install.installed(&output);
        assert!(
            said(&output.stdout).contains("the session went on"),
            "{output:?}"
        );
    });
}
