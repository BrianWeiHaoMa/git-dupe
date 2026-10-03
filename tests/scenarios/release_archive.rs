//! The release archive this check command made from a release build: exactly the release
//! executable, the page that build rendered, and the install note, under one directory;
//! and the note followed, as written, into the scenario's own temporary home, where
//! neither directory it names exists yet, after which Git finds git-dupe and `man` finds
//! its page there and nowhere else.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, asking_for_the_page, every_text_shown, holds_a_line_of_the_general_text, install_note,
    lines_of, note_commands, release_archive, under_each_release,
};

/// The directory every member lies in.
fn leading_directory() -> String {
    format!("git-dupe-{}", env!("CARGO_PKG_VERSION"))
}

/// One member of a tar archive as its header and its blocks give it.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Member {
    name: String,
    /// The header's type flag: `0` for a regular file.
    kind: u8,
    mode: u32,
    content: Vec<u8>,
}

/// Every member of a POSIX ustar archive, read from its headers: each header counts,
/// whatever it stands for, so that a directory entry or an extended header is a member
/// too.
fn members_of(archive: &[u8]) -> Vec<Member> {
    let mut members = Vec::new();
    let mut at = 0;
    loop {
        let header = archive
            .get(at..at + 512)
            .unwrap_or_else(|| panic!("the archive ends inside a header at byte {at}"));
        if header.iter().all(|&byte| byte == 0) {
            break;
        }
        assert_eq!(
            &header[257..265],
            b"ustar\x0000",
            "a ustar header at byte {at}"
        );
        let text = |start: usize, end: usize| {
            let field = &header[start..end];
            let used = field
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(field.len());
            String::from_utf8(field[..used].to_vec()).unwrap()
        };
        let octal = |start: usize, end: usize| {
            u32::from_str_radix(text(start, end).trim(), 8)
                .unwrap_or_else(|cause| panic!("a number at byte {}: {cause}", at + start))
        };
        let (prefix, name) = (text(345, 500), text(0, 100));
        let size = octal(124, 136) as usize;
        let content = archive[at + 512..at + 512 + size].to_vec();
        members.push(Member {
            name: if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            },
            kind: header[156],
            mode: octal(100, 108),
            content,
        });
        at += 512 + size.div_ceil(512) * 512;
    }
    assert!(
        archive[at..].iter().all(|&byte| byte == 0),
        "nothing after the end"
    );
    members
}

#[test]
fn the_archive_holds_exactly_the_release_executable_its_page_and_the_install_note() {
    let made = release_archive();
    let leading = leading_directory();
    let mut members = members_of(&fs::read(&made.path).unwrap());
    members.sort();
    let mut expected = [
        ("git-dupe", 0o755, &made.executable),
        ("git-dupe.1", 0o644, &made.page),
        ("INSTALL", 0o644, &install_note()),
    ]
    .map(|(name, mode, source)| Member {
        name: format!("{leading}/{name}"),
        kind: b'0',
        mode,
        content: fs::read(source).unwrap(),
    });
    expected.sort();
    // Compared without the contents first, so that a failure here shows the list.
    let listed = |members: &[Member]| -> Vec<(String, u8, u32)> {
        members
            .iter()
            .map(|member| (member.name.clone(), member.kind, member.mode))
            .collect()
    };
    assert_eq!(listed(&members), listed(&expected));
    for (member, source) in members.iter().zip(&expected) {
        assert!(
            member.content == source.content,
            "{} is not the bytes of its source",
            member.name
        );
    }

    // The release profile's executable and the page the build script renders from the
    // texts as they stand: the test build's executable differs, and its page does not.
    let release = &made.executable;
    assert!(
        release.ends_with("release/git-dupe"),
        "{}",
        release.display()
    );
    let tested = Path::new(env!("CARGO_BIN_EXE_git-dupe"));
    assert_ne!(fs::read(release).unwrap(), fs::read(tested).unwrap());
    let tested_page = Path::new(env!("OUT_DIR")).join("git-dupe.1");
    assert_eq!(
        fs::read(&made.page).unwrap(),
        fs::read(tested_page).unwrap()
    );
}

/// The words of a command line of the note: a note holding anything the shell would read
/// as more than plain words fails here, before a line of it runs.
fn plain_words(command: &str) -> Vec<&str> {
    assert!(
        command
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b" ~/._-".contains(&byte)),
        "more than plain words: {command}"
    );
    command.split_whitespace().collect()
}

/// The directory `word` names below `~/`, as the scenario's `home` gives it: a note naming
/// any other place fails here, before a line of it runs, and so never writes outside the
/// scenario's directory.
fn below_home(word: &str, home: &Path) -> PathBuf {
    let below = word
        .strip_prefix("~/")
        .map(|below| below.strip_suffix('/').unwrap_or(below))
        .filter(|below| {
            below
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
        })
        .unwrap_or_else(|| panic!("not a directory below ~/: {word}"));
    home.join(below)
}

/// Checks each command line of the note before any runs: a copy of a member into a
/// directory below `~/`, or the making of directories below `~/`.
fn only_copies_and_directories_below_home(commands: &[String], home: &Path) {
    assert!(!commands.is_empty(), "the note has no command line");
    for command in commands {
        match plain_words(command).as_slice() {
            ["cp", "git-dupe" | "git-dupe.1", directory] => {
                below_home(directory, home);
            }
            ["mkdir", "-p", directories @ ..] if !directories.is_empty() => {
                for directory in directories {
                    below_home(directory, home);
                }
            }
            _ => panic!("neither a copy of a member nor directories made: {command}"),
        }
    }
}

/// The directory the note copies `member` into, as the note writes it, without a
/// trailing `/`.
fn copied_into<'c>(commands: &'c [String], member: &str) -> &'c str {
    let copies: Vec<&String> = commands
        .iter()
        .filter(|command| command.split_whitespace().nth(1) == Some(member))
        .collect();
    let [command] = copies[..] else {
        panic!("not one copy of {member} in the note: {commands:?}");
    };
    let [_, _, directory] = plain_words(command)[..] else {
        panic!("not a copy of one file into one directory: {command}");
    };
    directory.strip_suffix('/').unwrap_or(directory)
}

/// The names in a directory.
fn names_in(directory: &Path) -> Vec<OsString> {
    let mut names: Vec<OsString> = fs::read_dir(directory)
        .unwrap_or_else(|cause| panic!("{}: {cause}", directory.display()))
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    names
}

#[test]
fn following_the_install_note_gives_git_dupe_and_its_page_from_a_temporary_home() {
    let made = release_archive();
    let version = concat!("git-dupe version ", env!("CARGO_PKG_VERSION"), "\n");
    under_each_release(|s| {
        // Unpacked by `tar`, as anyone unpacks it.
        let archive = made.path.as_os_str();
        let unpacked = s.dir().join("unpacked");
        fs::create_dir(&unpacked).unwrap();
        s.program("tar", [OsString::from("-xf"), archive.to_owned()])
            .from(&unpacked)
            .succeeds();
        let release = unpacked.join(leading_directory());
        let note = fs::read_to_string(release.join("INSTALL")).unwrap();
        let commands = note_commands(&note);

        // The directories the note copies into, a directory on `PATH` and the `man1`
        // directory of a manual path, both in the scenario's home and neither there yet,
        // as on a machine where nothing was installed below `~/.local`: what the note
        // needs, it makes.
        let home = s.dir().join("home");
        only_copies_and_directories_below_home(&commands, &home);
        let (bin, man1) = (
            copied_into(&commands, "git-dupe"),
            copied_into(&commands, "git-dupe.1"),
        );
        let manual_path = man1
            .strip_suffix("/man1")
            .unwrap_or_else(|| panic!("not a man1 directory: {man1}"));

        // The note says which directory must be on `PATH`, and which on the manual path:
        // the ones it copies into, which are the ones put there below, and no other.
        let said = note.split_whitespace().collect::<Vec<_>>().join(" ");
        for condition in [
            format!("{bin} on PATH"),
            format!("{manual_path} on the manual path"),
        ] {
            assert!(
                said.contains(&condition),
                "the note does not say {condition:?}"
            );
        }
        let [bin, man1, manual_path] = [bin, man1, manual_path].map(|word| below_home(word, &home));
        let manual_path = manual_path.as_path();
        for directory in [&bin, &man1] {
            assert!(
                fs::symlink_metadata(directory).is_err(),
                "{}",
                directory.display()
            );
        }

        // That directory, the release's `git`, and the caller's `PATH` without the build
        // directory and without any directory that holds a `git-dupe`: this machine may
        // have one installed, which would answer in place of the copy.
        let built = Path::new(env!("CARGO_BIN_EXE_git-dupe")).parent().unwrap();
        let callers = std::env::var_os("PATH").unwrap_or_default();
        let callers = std::env::split_paths(&callers)
            .filter(|dir| dir != built && fs::symlink_metadata(dir.join("git-dupe")).is_err());
        let release_bin = s.release_git().parent().unwrap().to_path_buf();
        let path =
            std::env::join_paths([bin.clone(), release_bin].into_iter().chain(callers)).unwrap();

        // Before the note is followed, nothing answers for git-dupe.
        let before = s.git(["dupe", "--version"]).variable("PATH", &path).run();
        assert_ne!(before.end, End::Code(0), "{before:?}");
        assert!(
            !before.stdout.starts_with(b"git-dupe version"),
            "{before:?}"
        );
        for absent in
            asking_for_the_page(s, manual_path).map(|asked| asked.variable("PATH", &path).run())
        {
            assert_ne!(absent.end, End::Code(0), "{absent:?}");
            assert!(
                !holds_a_line_of_the_general_text(&absent.stdout),
                "{absent:?}"
            );
        }

        // The note's lines, as written, from the directory that holds it.
        for command in &commands {
            s.program("/bin/sh", ["-c", command.as_str()])
                .from(&release)
                .variable("PATH", &path)
                .succeeds();
        }

        let after = s.git(["dupe", "--version"]).variable("PATH", &path).run();
        assert_eq!(after.end, End::Code(0), "{after:?}");
        assert_eq!(after.stdout, version.as_bytes(), "{after:?}");
        let [man, help] =
            asking_for_the_page(s, manual_path).map(|asked| asked.variable("PATH", &path).run());
        assert_eq!(man.end, End::Code(0), "{man:?}");
        every_text_shown(&lines_of(&man.stdout), "the installed `man git-dupe`");
        assert_eq!(help.end, End::Code(0), "{help:?}");
        assert_eq!(help.stdout, man.stdout, "`git dupe --help`: {help:?}");

        assert_eq!(names_in(&bin), ["git-dupe"]);
        assert_eq!(names_in(&man1), ["git-dupe.1"]);
        assert_eq!(
            fs::read(bin.join("git-dupe")).unwrap(),
            fs::read(&made.executable).unwrap()
        );
    });
}
