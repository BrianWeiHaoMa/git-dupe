//! The run: its words, its outcomes, its end, and what it changes of the environment.

use super::*;
use crate::links_and_modes::{names_every_byte_string, signals};
use std::path::PathBuf;

#[test]
fn an_argument_the_kernel_refuses_is_the_too_long_outcome_and_nothing_runs() {
    // One argument every system refuses (S2, S20): above Linux's 128 KiB for one
    // argument, macOS's 1 MiB for all of them together, and Windows's 32,767 units for
    // the whole command line.
    let argument = "a".repeat(2 * 1024 * 1024);
    for captured in [false, true] {
        let mut run = Run::public([argument.as_str()]);
        if captured {
            run = run.capture_output().capture_errors();
        }
        let run = run.start();
        assert!(matches!(run, Err(Failure::TooLong)), "{:?}", run.err());
    }
}

fn owned(words: &[&str]) -> Vec<Word> {
    words.iter().map(Word::from).collect()
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
fn a_runs_own_variable_reaches_its_git_alone_over_the_value_received() {
    // The alias prints a variable as its `git` received it: the run's own value there,
    // the value this process received in every other run. Cargo gives every check this
    // one; the shell of Git for Windows rewrites `HOME` and `PATH` as it starts.
    let name = "CARGO_PKG_NAME";
    let printed = |run: Run| {
        let finished = run.capture_output().start().expect("a run");
        assert_eq!(finished.end, End::Code(0));
        finished.stdout
    };
    let alias = format!("alias.print=!printf %s \"${name}\"");
    let print = || Run::public(["-c", alias.as_str(), "print"]);
    let received = std::env::var_os(name).unwrap_or_default();
    let mut own = received.clone();
    own.push("-elsewhere");
    assert_eq!(
        printed(print().variable(name, own.as_os_str())),
        bytes::of(&own)
    );
    assert_eq!(printed(print()), bytes::of(&received));
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
    if !signals() {
        return;
    }
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

#[test]
fn a_status_is_its_signal_or_the_low_byte_of_its_code() {
    // `0xC000013A`, the status of a process Ctrl-C ended on Windows, as Git reduces it.
    assert_eq!(low_byte(0xC000_013A_u32 as i32), 0x3A);
    assert_eq!(low_byte(0x13A), 0x3A);
    assert_eq!(low_byte(137), 137);
    assert_eq!(low_byte(1), 1);
    for code in ["137", "1"] {
        let alias = format!("alias.ended=!exit {code}");
        let run = Run::public(["-c", alias.as_str(), "ended"]).start();
        assert_eq!(
            run.expect("a run").end,
            End::Code(code.parse().expect("a number"))
        );
    }
    if signals() {
        let run = Run::public(["-c", "alias.die=!kill -TERM $PPID", "die"]).start();
        assert_eq!(run.expect("a run").end, End::Signal(15));
    }
}

#[test]
fn a_failed_spawn_is_too_long_for_the_two_numbers_that_say_so_and_no_other() {
    let too_long = io::Error::from(io::ErrorKind::ArgumentListTooLong);
    assert!(matches!(spawn_failure(too_long), Failure::TooLong));
    let too_long = io::Error::from_raw_os_error(206);
    assert!(matches!(spawn_failure(too_long), Failure::TooLong));
    let missing = io::Error::from(io::ErrorKind::NotFound);
    assert!(matches!(spawn_failure(missing), Failure::Other(_)));
}

#[test]
fn a_program_that_is_not_there_is_no_list_too_long() {
    let directory = std::env::temp_dir().join(format!("git-dupe-no-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("a directory");
    let cause = Command::new(directory.join("git")).spawn().err();
    std::fs::remove_dir_all(&directory).expect("removed");
    let cause = cause.expect("no such program");
    assert!(matches!(spawn_failure(cause), Failure::Other(_)));
}

#[test]
fn a_word_that_is_no_name_ends_the_run_naming_its_path_and_it_is_not_spawned() {
    let path = b"notes/\xff";
    let words = vec![
        Word::from("ls-files"),
        Word::from("--"),
        Word::naming([&b":(top,literal)"[..], path].concat(), path),
    ];
    match Run::public(words).capture_output().capture_errors().start() {
        Err(Failure::NotAName(named)) => {
            assert!(!names_every_byte_string());
            assert_eq!(named, path);
        }
        Err(failure) => panic!("{failure:?}"),
        // The word reached Git whole: outside a repository too it was read.
        Ok(_) => assert!(names_every_byte_string()),
    }
}

#[test]
fn a_plain_word_names_itself_and_a_variable_crosses_as_a_word_does() {
    if names_every_byte_string() {
        return;
    }
    let word = Run::public([&b"\xff"[..]]).command().map(|_| ());
    assert!(matches!(word, Err(Failure::NotAName(named)) if named == b"\xff"));
    let variable = Run::public(["version"]).variable("GIT_NAMESPACE", &b"x\xff"[..]);
    let variable = variable.command().map(|_| ());
    assert!(matches!(variable, Err(Failure::NotAName(named)) if named == b"x\xff"));
}

#[test]
fn a_private_run_changes_the_locating_variables_on_the_inherited_environment_alone() {
    let private = PathBuf::from("private");
    let root = PathBuf::from("root");
    let run = Run::private(&private, &root, ["cat-file"]).variable("GIT_GRAFT_FILE", "/dev/null");
    let command = run.command().expect("a command");
    let mut envs: Vec<(String, Option<String>)> = command
        .get_envs()
        .map(|(name, value)| {
            let text = |name: &OsStr| name.to_string_lossy().into_owned();
            (text(name), value.map(text))
        })
        .collect();
    envs.sort();
    let removed = |name: &str| (name.to_string(), None);
    let set = |name: &str, value: &str| (name.to_string(), Some(value.to_string()));
    assert_eq!(
        envs,
        [
            removed("GIT_ALTERNATE_OBJECT_DIRECTORIES"),
            removed("GIT_COMMON_DIR"),
            set("GIT_DIR", "private"),
            set("GIT_GRAFT_FILE", "/dev/null"),
            removed("GIT_INDEX_FILE"),
            removed("GIT_OBJECT_DIRECTORY"),
            removed("GIT_REFERENCE_BACKEND"),
            set("GIT_WORK_TREE", "root"),
        ]
    );
}
