//! What a scenario is given: a fresh directory, and `git` as the release under test in an
//! environment built from nothing.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use super::fresh::FreshDirectory;
use super::output::{End, Output};
use super::provision::Store;
use super::releases::{NARROWING_VARIABLE, selection};
use super::running::Running;

/// A release this run exercises, built and present.
struct Release {
    version: &'static str,
    /// Canonical, for comparison with the paths its `git` reports.
    directory: PathBuf,
}

/// Runs `scenario` once under each release this run exercises. A failure under one
/// release does not stop the others; the check then fails naming every release it
/// failed under.
pub fn under_each_release(scenario: impl Fn(&Scenario)) {
    let mut failed = Vec::new();
    for release in selected() {
        let run = catch_unwind(AssertUnwindSafe(|| scenario(&Scenario::begin(release))));
        if run.is_err() {
            eprintln!("the failure above was under Git {}", release.version);
            failed.push(release.version);
        }
    }
    assert!(
        failed.is_empty(),
        "the scenario failed under Git {}",
        failed.join(", ")
    );
}

/// The releases of this run, selected and provided once for the whole check command.
fn selected() -> &'static [Release] {
    static SELECTED: OnceLock<Result<Vec<Release>, String>> = OnceLock::new();
    match SELECTED.get_or_init(select_and_provide) {
        Ok(releases) => releases,
        Err(cause) => panic!("{cause}"),
    }
}

fn select_and_provide() -> Result<Vec<Release>, String> {
    let versions = selection(std::env::var_os(NARROWING_VARIABLE).as_deref())?;
    report(&format!("under Git {}", versions.join(", ")));
    let store = Store::of_the_repository();
    let absent = store.absent(&versions);
    if !absent.is_empty() {
        report(&format!("obtaining and building Git {}", absent.join(", ")));
    }
    store.provide(&versions)?;
    versions
        .into_iter()
        .map(|version| {
            let directory = store.release_directory(version);
            let directory = fs::canonicalize(&directory)
                .map_err(|cause| format!("{}: {cause}", directory.display()))?;
            Ok(Release { version, directory })
        })
        .collect()
}

/// Says, past the capture of a check's output, what this run is doing or what a scenario
/// measured, so that a passing run shows it too.
pub fn report(text: &str) {
    let _ = writeln!(std::io::stderr(), "scenarios: {text}");
}

pub struct Scenario {
    directory: FreshDirectory,
    environment: Vec<(OsString, OsString)>,
    /// The release's own `git`, behind everything the scenario's `PATH` can put first.
    release_git: PathBuf,
}

impl Scenario {
    /// The one place that builds a scenario's environment, and checks it.
    fn begin(release: &Release) -> Scenario {
        let directory = FreshDirectory::create();
        let home = directory.path().join("home");
        fs::create_dir(&home).unwrap_or_else(|cause| panic!("{}: {cause}", home.display()));
        // Discovery stops below the ceiling, never at it: naming the directory above the
        // scenario's own keeps Git from finding a repository that holds it.
        let ceiling = directory
            .path()
            .parent()
            .expect("a directory below the temporary one");

        let built = Path::new(env!("CARGO_BIN_EXE_git-dupe"));
        let callers = std::env::var_os("PATH").unwrap_or_default();
        let front = [
            release.directory.join("bin"),
            built
                .parent()
                .expect("the built git-dupe lies in a directory")
                .to_path_buf(),
        ];
        let path = std::env::join_paths(front.into_iter().chain(std::env::split_paths(&callers)))
            .unwrap_or_else(|cause| panic!("PATH: {cause}"));

        let scenario = Scenario {
            environment: vec![
                ("PATH".into(), path),
                ("HOME".into(), home.into()),
                ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
                ("GIT_SSH_COMMAND".into(), "false".into()),
                ("GIT_CEILING_DIRECTORIES".into(), ceiling.into()),
            ],
            directory,
            release_git: release.directory.join("bin/git"),
        };
        scenario.check_git_is(release);
        scenario
    }

    /// `git` found through the scenario's `PATH` must be the release under test, with
    /// that release's own programs behind it, and not the machine's.
    fn check_git_is(&self, release: &Release) {
        let version = self.git(["--version"]).run();
        assert!(
            version.end == End::Code(0)
                && version.stdout == format!("git version {}\n", release.version).as_bytes(),
            "`git --version` is not Git {}: {version:?}",
            release.version
        );
        let exec_path = self.git(["--exec-path"]).run();
        let reported = Path::new(OsStr::from_bytes(exec_path.stdout.trim_ascii_end()));
        assert!(
            exec_path.end == End::Code(0)
                && fs::canonicalize(reported)
                    .is_ok_and(|path| path.starts_with(&release.directory)),
            "`git --exec-path` is not inside {}: {exec_path:?}",
            release.directory.display()
        );
    }

    /// The scenario's own directory: fresh, outside this repository, with no repository
    /// above it that Git can discover. Its `home` is the scenario's `HOME`.
    pub fn dir(&self) -> &Path {
        self.directory.path()
    }

    /// The path of the release's own `git` program: for a scenario or support that puts
    /// something else first where Git looks for `git`, or builds a `PATH` of its own, and
    /// must still reach the release.
    pub fn release_git(&self) -> &Path {
        &self.release_git
    }

    /// `git <words>`, to be run from the scenario's directory unless told otherwise. The
    /// words are bytes. git-dupe is reached as `git dupe …`, never run directly.
    pub fn git<W: AsRef<OsStr>>(&self, words: impl IntoIterator<Item = W>) -> Git<'_> {
        self.program("git", words)
    }

    /// `<program> <words>` in the scenario's environment, as `git` runs: for a program a
    /// scenario needs beside Git, such as `man` or `tar`, found on the scenario's `PATH`.
    pub fn program<W: AsRef<OsStr>>(
        &self,
        program: impl AsRef<OsStr>,
        words: impl IntoIterator<Item = W>,
    ) -> Git<'_> {
        Git {
            scenario: self,
            program: program.as_ref().to_owned(),
            words: words
                .into_iter()
                .map(|word| word.as_ref().to_owned())
                .collect(),
            from: self.dir().to_path_buf(),
            variables: Vec::new(),
            stdout_to: None,
            input: None,
            file_size_limit: None,
            file_size_signal_ignored: false,
            leaving_no_process: false,
        }
    }
}

#[cfg(test)]
impl Scenario {
    /// A scenario under the first release of this run: for a check of the harness itself.
    pub(super) fn of_the_first_release() -> Scenario {
        Scenario::begin(&selected()[0])
    }
}

/// One run of `git`, or of another program of the scenario's, not yet started.
pub struct Git<'s> {
    scenario: &'s Scenario,
    program: OsString,
    words: Vec<OsString>,
    from: PathBuf,
    variables: Vec<(OsString, OsString)>,
    stdout_to: Option<PathBuf>,
    input: Option<Vec<u8>>,
    file_size_limit: Option<u32>,
    file_size_signal_ignored: bool,
    leaving_no_process: bool,
}

impl Git<'_> {
    /// The directory to run from.
    pub fn from(mut self, directory: &Path) -> Self {
        self.from = directory.to_path_buf();
        self
    }

    /// A variable beside the scenario's environment, for this run.
    pub fn variable(mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.variables
            .push((name.as_ref().to_owned(), value.as_ref().to_owned()));
        self
    }

    /// A file to open for the run's standard output, in place of capturing it: a device
    /// that refuses every write, for one. `Output::stdout` is then empty.
    pub fn standard_output_to(mut self, file: &Path) -> Self {
        self.stdout_to = Some(file.to_path_buf());
        self
    }

    /// Bytes to feed on the run's standard input, in place of no input.
    pub fn input(mut self, bytes: &[u8]) -> Self {
        self.input = Some(bytes.to_vec());
        self
    }

    /// Starts it through `/bin/sh` under a soft limit of `blocks`, in the shell's blocks, on
    /// the size of any file a process of the run writes, and with no core file: a process
    /// that writes past the limit dies of `SIGXFSZ`. A process of the run may raise the
    /// soft limit again for itself and what it starts.
    pub(super) fn file_size_limit(mut self, blocks: u32) -> Self {
        self.file_size_limit = Some(blocks);
        self
    }

    /// Under `file_size_limit`, ignores `SIGXFSZ` for the run, so that a write past the
    /// limit fails with `EFBIG` and the writer goes on, where it would have been killed.
    pub(super) fn file_size_signal_ignored(mut self) -> Self {
        self.file_size_signal_ignored = true;
        self
    }

    /// Starts it in a process group of its own and, once it has ended, waits until no
    /// process of that group is left running: for a run in which a process is killed, so
    /// that nothing it started is still at work when the scenario takes its next step.
    pub(super) fn leaving_no_process(mut self) -> Self {
        self.leaving_no_process = true;
        self
    }

    /// Starts it and returns at once, in a process group of its own, which is waited for
    /// as `leaving_no_process` says: for a scenario that acts while the run goes on, and
    /// then waits for it as `run` does (`Running`).
    pub fn start(mut self) -> Running {
        self.leaving_no_process = true;
        self.begin()
    }

    /// Runs it to its end with the supplied standard input, or none. No variable of the
    /// caller's reaches it but the `PATH` behind the release and the built git-dupe.
    pub fn run(self) -> Output {
        self.begin().wait()
    }

    fn begin(self) -> Running {
        let named = self.named();
        let mut command = match self.file_size_limit {
            None => Command::new(&self.program),
            Some(blocks) => {
                let mut shell = Command::new("/bin/sh");
                let ignoring = if self.file_size_signal_ignored {
                    "trap '' XFSZ && "
                } else {
                    ""
                };
                shell.args([
                    "-c",
                    &format!(r#"{ignoring}ulimit -S -c 0 && ulimit -S -f "$0" && exec "$@""#),
                    &blocks.to_string(),
                ]);
                shell.arg(&self.program);
                shell
            }
        };
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(file) = &self.stdout_to {
            let opened = fs::OpenOptions::new().write(true).open(file);
            command.stdout(opened.unwrap_or_else(|cause| panic!("{}: {cause}", file.display())));
        }
        command
            .args(&self.words)
            .current_dir(&self.from)
            .env_clear()
            .envs(
                self.scenario
                    .environment
                    .iter()
                    .map(|(name, value)| (name, value)),
            )
            .envs(self.variables.iter().map(|(name, value)| (name, value)))
            .stdin(if self.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        Running::start(command, self.input, named, self.leaving_no_process)
    }

    /// Runs it as `run` does; it must exit 0.
    pub fn succeeds(self) -> Output {
        let named = self.named();
        let output = self.run();
        assert_eq!(output.end, End::Code(0), "{named}: {output:?}");
        output
    }

    /// The program and its words, for a message.
    fn named(&self) -> String {
        format!("{} {:?}", self.program.to_string_lossy(), self.words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the caller's environment is clean, the isolation scenarios pass whatever the
    /// harness inherits. Here they run again as a check command of their own whose caller
    /// is hostile: a repository above the temporary directory, and every variable that
    /// can name a repository, Git's programs, or a configuration file naming one.
    #[test]
    fn a_hostile_caller_changes_nothing_a_scenario_sees() {
        let scenario = Scenario::begin(&selected()[0]);
        let above = scenario.dir().join("above");
        let made = scenario
            .git([OsStr::new("init"), OsStr::new("-q"), above.as_os_str()])
            .run();
        assert_eq!(made.end, End::Code(0), "{made:?}");
        let temporary = above.join("temporary");
        let global = scenario.dir().join("global");
        let xdg = scenario.dir().join("xdg");
        fs::create_dir(&temporary).unwrap();
        fs::create_dir_all(xdg.join("git")).unwrap();
        fs::write(&global, "[caller]\n\tglobal = set\n").unwrap();
        fs::write(xdg.join("git/config"), "[caller]\n\txdg = set\n").unwrap();

        let rerun = Command::new(std::env::current_exe().unwrap())
            .arg("isolation::")
            .env("TMPDIR", &temporary)
            .env("GIT_DIR", above.join(".git"))
            .env("GIT_EXEC_PATH", scenario.dir())
            .env("GIT_CONFIG_GLOBAL", &global)
            .env("XDG_CONFIG_HOME", &xdg)
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&rerun.stdout);
        // Success alone is not enough: a filter that matches no scenario succeeds too.
        assert!(
            rerun.status.success() && said.contains("test isolation::"),
            "{said}{}",
            String::from_utf8_lossy(&rerun.stderr)
        );
    }
}
