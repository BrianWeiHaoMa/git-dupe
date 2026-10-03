//! Runner: locating the workspace and running Git.
//!
//! `Run::start` below is the one place in the executable that starts a process, and
//! `git`, found on `PATH` as the caller's, is the only program it starts. A run is
//! described before it starts: the repository it is made against, its directory, its
//! words as bytes, whether its command word is the user's, which streams it captures, the
//! bytes it is fed, whether its paths are git-dupe's own, and whether it reads the index
//! the user's own command reads. Its environment is built by `environment` alone, never
//! by a caller (G10).
//!
//! A run whose command word is the user's carries `-c help.autocorrect=0` before that
//! word, written by `arguments` and nowhere else, so that a word Git does not know is
//! never run as another command, whatever the configuration or a `-c` before `dupe`
//! sets (G19, S10). Nothing about its environment is decided from that word.
//!
//! A command an alias chain reached carries the chain's **prefix**, the global options
//! its expansions began with, before the command word of every run (`Holds/G19`). The
//! front hands it over once, by `carry_prefix`, when it dispatches the command, and every
//! run started after that carries it, the keeper's and settle's included: no run site
//! names it, so none can leave it out, and a command no chain reached has none. A lookup
//! of the chain itself carries the prefix gathered so far, by `prefixed`. The prefix
//! stands apart from a run's words, because `environment` reads a run's first word.
//!
//! What a run answers is read in one of Git's machine forms (R5). `records` is the home
//! of a reader of such a form, below every part that asks; a form that one caller alone
//! reads, with checks of its own, may stay with that caller.

pub mod environment;
pub mod locate;
pub mod records;

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::thread;

pub use environment::Against;

/// How a Git child ended.
#[derive(Debug, PartialEq, Eq)]
pub enum End {
    Code(u8),
    Signal(i32),
}

/// A run that started and ended, with what was captured of it. A stream that was not
/// captured is empty here.
pub struct Finished {
    pub end: End,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// A run that did not start, or whose fed input could not be written whole to a child
/// that then ended other than by a signal.
#[derive(Debug)]
pub enum Failure {
    /// The argument list did not fit, and nothing ran. Distinct, because a list of paths
    /// is never shortened to make it fit.
    TooLong,
    Other(io::Error),
}

/// The prefix of the command an alias chain was dispatched as, once it is.
static CARRIED: OnceLock<Vec<OsString>> = OnceLock::new();

/// Makes `prefix` the prefix of every run started from now on: the global options an
/// alias chain set before the command it was dispatched as (`Holds/G19`). A command is
/// dispatched once, so a second prefix is a fault of the caller.
pub fn carry_prefix(prefix: Vec<OsString>) {
    assert!(
        CARRIED.set(prefix).is_ok(),
        "a command is dispatched once, with one prefix"
    );
}

/// One run of `git`, not yet started.
pub struct Run<'r> {
    against: Against<'r>,
    prefix: &'r [OsString],
    words: Vec<OsString>,
    directory: Option<&'r Path>,
    capture_output: bool,
    capture_errors: bool,
    input: Option<Vec<u8>>,
    own_paths: bool,
    users_command: bool,
    users_index: bool,
}

impl<'r> Run<'r> {
    /// A run against the public repository, from the user's directory, with every
    /// stream inherited.
    pub fn public<W: AsRef<OsStr>>(words: impl IntoIterator<Item = W>) -> Run<'r> {
        Run::against(Against::Public, words)
    }

    /// A run against the private repository at `git_directory`, whose working tree is
    /// `root`, both absolute, from the user's directory, with every stream inherited.
    pub fn private<W: AsRef<OsStr>>(
        git_directory: &'r Path,
        root: &'r Path,
        words: impl IntoIterator<Item = W>,
    ) -> Run<'r> {
        Run::against(
            Against::Private {
                git_directory,
                root,
            },
            words,
        )
    }

    /// A run against the repository `against` names, from the user's directory, with
    /// every stream inherited.
    pub fn against<W: AsRef<OsStr>>(
        against: Against<'r>,
        words: impl IntoIterator<Item = W>,
    ) -> Self {
        Run {
            against,
            prefix: &[],
            words: words
                .into_iter()
                .map(|word| word.as_ref().to_owned())
                .collect(),
            directory: None,
            capture_output: false,
            capture_errors: false,
            input: None,
            own_paths: false,
            users_command: false,
            users_index: false,
        }
    }

    /// Global options before the words, after any carried prefix: for a lookup of an
    /// alias chain, the options its expansions have set so far (`Holds/G19`); for the
    /// status `detach` reads, `--no-optional-locks` (`Holds/G3`).
    pub fn prefixed(mut self, prefix: &'r [OsString]) -> Self {
        self.prefix = prefix;
        self
    }

    /// Runs from `directory`, the root, instead of the user's directory.
    pub fn from(mut self, directory: &'r Path) -> Self {
        self.directory = Some(directory);
        self
    }

    /// Standard output to the caller, as bytes: for a run whose answer is read.
    pub fn capture_output(mut self) -> Self {
        self.capture_output = true;
        self
    }

    /// Standard error to the caller, as bytes: for a run whose failure is an answer
    /// rather than an error.
    pub fn capture_errors(mut self) -> Self {
        self.capture_errors = true;
        self
    }

    /// Bytes for standard input, written while the output is read, so that a list larger
    /// than a pipe's buffer cannot stall the run. Without them standard input is
    /// inherited.
    pub fn feed(mut self, input: Vec<u8>) -> Self {
        self.input = Some(input);
        self
    }

    /// The run's pathspecs or paths are git-dupe's own, so the pathspec variables of a
    /// global option before `dupe` do not apply to them.
    pub fn own_paths(mut self) -> Self {
        self.own_paths = true;
        self
    }

    /// The run's first word is a command the user typed: a word passed through, `stash`,
    /// `push`, `pull`, `fetch`, `remote`, the first word of `git dupe git`, or the `status`,
    /// `add`, or `clean` git-dupe runs for the user's word. It gets `-c help.autocorrect=0`
    /// before its words, and no environment rule reads its first word: the user's own
    /// `config` keeps `GIT_CONFIG`, as plain `git config` does.
    pub fn users_command(mut self) -> Self {
        self.users_command = true;
        self
    }

    /// The run, a public listing of git-dupe's own, reads the index the user's own command
    /// reads: the one a received `GIT_INDEX_FILE` names, as a hook's does (S9), else the
    /// public index. For the listing `clean` asks before its `clean` runs, so that both
    /// decide by one index (`Holds/G16`). A private run never reads it (G10).
    pub fn in_the_users_index(mut self) -> Self {
        self.users_index = true;
        self
    }

    /// Runs it to its end.
    pub fn start(self) -> Result<Finished, Failure> {
        let own_command = if self.users_command {
            None
        } else {
            self.words.first().map(OsString::as_os_str)
        };
        let environment = environment::of(
            &self.against,
            own_command,
            self.own_paths,
            self.users_index,
            std::env::vars_os(),
            std::env::current_dir,
        )
        .map_err(Failure::Other)?;
        let carried = CARRIED.get().map_or(&[][..], Vec::as_slice);
        let words = arguments(
            [carried, self.prefix].concat(),
            self.users_command,
            self.words,
        );
        let piped = |captured: bool| {
            if captured {
                Stdio::piped()
            } else {
                Stdio::inherit()
            }
        };
        let mut command = Command::new("git");
        command
            .args(&words)
            .env_clear()
            .envs(environment)
            .stdin(piped(self.input.is_some()))
            .stdout(piped(self.capture_output))
            .stderr(piped(self.capture_errors));
        if let Some(directory) = self.directory {
            command.current_dir(directory);
        }
        let mut child = command.spawn().map_err(|cause| match cause.kind() {
            io::ErrorKind::ArgumentListTooLong => Failure::TooLong,
            _ => Failure::Other(cause),
        })?;
        let feeder = match (self.input, child.stdin.take()) {
            (Some(input), Some(mut stdin)) => Some(thread::spawn(move || stdin.write_all(&input))),
            _ => None,
        };
        let output = child.wait_with_output().map_err(Failure::Other)?;
        let end = end(output.status)?;
        if let Some(feeder) = feeder {
            let fed = feeder
                .join()
                .map_err(|_| Failure::Other(io::Error::other("feeding git's input failed")))?;
            // An answer to part of the input is no answer: the run failed. A child killed
            // by a signal ended by it, whatever it had read, and the command ends as it
            // did (`Composition/Front`, "Lines").
            if !matches!(end, End::Signal(_)) {
                fed.map_err(Failure::Other)?;
            }
        }
        Ok(Finished {
            end,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

/// The arguments of a run: the prefix, then, when the command word is the user's,
/// `-c help.autocorrect=0`, then the words. A `-c` on the command line outranks the
/// configuration and every earlier `-c`, a `-c` before `dupe` and one of the prefix
/// included (S10). This is the one place `help.autocorrect` is written.
fn arguments(prefix: Vec<OsString>, users_command: bool, words: Vec<OsString>) -> Vec<OsString> {
    let autocorrect = ["-c", "help.autocorrect=0"].map(OsString::from);
    let autocorrect = if users_command { &autocorrect[..] } else { &[] };
    prefix
        .into_iter()
        .chain(autocorrect.iter().cloned())
        .chain(words)
        .collect()
}

fn end(status: ExitStatus) -> Result<End, Failure> {
    if let Some(signal) = status.signal() {
        return Ok(End::Signal(signal));
    }
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map(End::Code)
        .ok_or_else(|| Failure::Other(io::Error::other(format!("git ended as {status}"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_argument_the_kernel_refuses_is_the_too_long_outcome_and_nothing_runs() {
        // One argument above the kernel's limit for a single one (32 pages, 128 KiB).
        let argument = "a".repeat(200 * 1024);
        for captured in [false, true] {
            let mut run = Run::public([&argument]);
            if captured {
                run = run.capture_output().capture_errors();
            }
            let run = run.start();
            assert!(matches!(run, Err(Failure::TooLong)), "{:?}", run.err());
        }
    }

    fn owned(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_users_command_has_autocorrect_off_before_its_first_word() {
        let words = owned(&["stahs", "-u", "-c", "x=y"]);
        assert_eq!(
            arguments(Vec::new(), true, words),
            owned(&["-c", "help.autocorrect=0", "stahs", "-u", "-c", "x=y"])
        );
        assert_eq!(
            arguments(Vec::new(), true, Vec::new()),
            owned(&["-c", "help.autocorrect=0"])
        );
    }

    #[test]
    fn a_prefix_stands_before_autocorrect_and_the_words_and_never_among_them() {
        let prefix = owned(&["-c", "status.short=true", "--exec-path=/x", "-C", ""]);
        assert_eq!(
            arguments(prefix.clone(), true, owned(&["status", "-b"])),
            owned(&[
                "-c",
                "status.short=true",
                "--exec-path=/x",
                "-C",
                "",
                "-c",
                "help.autocorrect=0",
                "status",
                "-b"
            ])
        );
        assert_eq!(
            arguments(prefix, false, owned(&["ls-files", "-z"])),
            owned(&[
                "-c",
                "status.short=true",
                "--exec-path=/x",
                "-C",
                "",
                "ls-files",
                "-z"
            ])
        );
        assert_eq!(
            arguments(Vec::new(), false, owned(&["config"])),
            owned(&["config"])
        );
    }

    #[test]
    fn fed_input_larger_than_a_pipe_reaches_git_whole_while_its_output_is_read() {
        // `hash-object --stdin` answers only after reading all of its input; `-w` is
        // absent, so nothing is written.
        let input = vec![b'x'; 4 * 1024 * 1024];
        let fed = Run::public(["hash-object", "--stdin"])
            .feed(input.clone())
            .capture_output()
            .start()
            .expect("a run");
        let told = Run::public(["hash-object", "--stdin"])
            .feed([&input[..], b"y"].concat())
            .capture_output()
            .start()
            .expect("a run");
        assert_eq!(fed.end, End::Code(0));
        assert_eq!(fed.stdout.len(), 41, "{}", fed.stdout.escape_ascii());
        assert_ne!(fed.stdout, told.stdout);
    }

    #[test]
    fn a_fed_run_killed_by_a_signal_ends_by_the_signal_whatever_it_read() {
        // The alias kills the `git` that runs it before anything reads the input, so the
        // input, larger than a pipe holds, cannot be written whole.
        let fed = Run::public(["-c", "alias.die=!kill -TERM $PPID", "die"])
            .feed(vec![b'x'; 4 * 1024 * 1024])
            .capture_output()
            .capture_errors()
            .start()
            .expect("a run that ended");
        assert_eq!(fed.end, End::Signal(15));
    }
}
