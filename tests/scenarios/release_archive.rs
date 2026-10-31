//! The release archive this check command made from a release build, named for the
//! platform and the architecture of the machine that made it: exactly the release
//! executable, on Linux and macOS the page that build rendered, and the platform's install
//! note, under one directory, with its `.sha256` beside it; and the note followed, as
//! written, into the scenario's own temporary home, where no directory it names exists
//! yet, after which Git finds git-dupe, and on Linux and macOS `man` finds its page, there
//! and nowhere else.

use std::env::consts::EXE_SUFFIX;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Scenario, architecture, archive_extension, asking_for_the_page, every_text_shown,
    hex_digest, holds_a_line_of_the_general_text, install_note, lines_of, note_commands,
    path_for_an_install, release_archive, under_each_release, windows_install_note,
    windows_powershell, word, zip_members,
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

/// One archive per platform and architecture (P3): its name carries the platform's word
/// and the machine's architecture as the platform module gives them, and it stands
/// beside the executable it packs.
#[test]
fn the_archive_is_named_for_the_platform_and_the_architecture_beside_the_executable() {
    let made = release_archive();
    let name = format!(
        "{}-{}-{}.{}",
        leading_directory(),
        word(),
        architecture(),
        archive_extension()
    );
    assert_eq!(made.path, made.executable.with_file_name(&name));
    assert!(made.path.is_file(), "{}", made.path.display());
}

/// The `.sha256` names the archive by its bare name, in the one line `shasum -c` reads, and
/// no packing name outlives the making: of the names that begin with the archive's, only
/// these two stand beside the executable (P3).
#[test]
fn the_digest_beside_the_archive_names_it_by_its_bare_name() {
    let made = release_archive();
    let bare = made.path.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        made.digest,
        made.path.with_file_name(format!("{bare}.sha256"))
    );
    let archive = fs::read(&made.path).unwrap();
    assert_eq!(
        fs::read_to_string(&made.digest).unwrap(),
        format!("{}  {bare}\n", hex_digest(&archive))
    );
    let beside = names_in(made.path.parent().unwrap());
    let beginning_alike: Vec<&OsString> = beside
        .iter()
        .filter(|name| name.to_string_lossy().starts_with(bare))
        .collect();
    assert_eq!(
        beginning_alike,
        [
            &OsString::from(bare),
            &OsString::from(format!("{bare}.sha256"))
        ]
    );
}

#[test]
fn the_archive_holds_exactly_the_release_executable_its_page_and_the_install_note() {
    let made = release_archive();
    let archive = fs::read(&made.path).unwrap();
    match word() {
        "windows" => the_zip_holds_exactly_the_executable_and_the_note(&archive),
        _ => the_tar_holds_exactly_the_three_members(&archive),
    }

    // The release profile's executable and the page the build script renders from the
    // texts as they stand: the test build's executable differs, and its page does not.
    let release = &made.executable;
    let name = format!("git-dupe{EXE_SUFFIX}");
    assert!(
        release.ends_with(Path::new("release").join(name)),
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

fn the_tar_holds_exactly_the_three_members(archive: &[u8]) {
    let made = release_archive();
    let leading = leading_directory();
    let mut members = members_of(archive);
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
}

/// Exactly two stored entries, no directory entry and no page (P3, F10), and the executable
/// the static C runtime reached: its bytes are read, never the configuration, because a
/// `RUSTFLAGS` of the caller's replaces the configuration's, and a dynamic build runs here
/// and fails on a machine without the runtime (S25).
fn the_zip_holds_exactly_the_executable_and_the_note(archive: &[u8]) {
    let made = release_archive();
    let leading = leading_directory();
    let mut members = zip_members(archive);
    members.sort();
    let listed: Vec<(&str, u16)> = members
        .iter()
        .map(|member| (member.name.as_str(), member.method))
        .collect();
    let executable = format!("{leading}/git-dupe.exe");
    let note = format!("{leading}/INSTALL-windows.txt");
    assert_eq!(listed, [(&*note, 0), (&*executable, 0)]);
    assert!(
        members[0].content == fs::read(windows_install_note()).unwrap(),
        "the note is not the bytes of its source"
    );
    assert!(
        members[1].content == fs::read(&made.executable).unwrap(),
        "the executable is not the bytes of the release build"
    );
    for runtime in ["VCRUNTIME140", "api-ms-win-crt", "MSVCP"] {
        let needle = runtime.as_bytes();
        assert!(
            !members[1]
                .content
                .windows(needle.len())
                .any(|bytes| bytes.eq_ignore_ascii_case(needle)),
            "the packed executable names the dynamic C runtime's {runtime}"
        );
    }
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

/// The one place the Windows note may name: L4's, however the note spells the copy.
const WINDOWS_BIN: &str = r"~\.local\bin";

/// What is wrong with the Windows note's command lines, read before any runs: only plain
/// words, and only the directory made and the executable copied into `~\.local\bin`, each
/// once and in that order. A line naming any other place would install elsewhere (L4).
fn windows_note_fault(commands: &[String]) -> Option<String> {
    let plain = |byte: u8| byte.is_ascii_alphanumeric() || br" ~\._-".contains(&byte);
    if let Some(line) = commands.iter().find(|line| !line.bytes().all(plain)) {
        return Some(format!("more than plain words: {line}"));
    }
    let lines: Vec<Vec<&str>> = commands
        .iter()
        .map(|line| line.split_whitespace().collect())
        .collect();
    let copied_into = format!(r"{WINDOWS_BIN}\");
    let expected: [&[&str]; 2] = [
        &["New-Item", "-ItemType", "Directory", "-Force", WINDOWS_BIN],
        &["Copy-Item", "git-dupe.exe", &copied_into],
    ];
    (lines != expected).then(|| format!("not the two lines of {WINDOWS_BIN}: {commands:?}"))
}

/// Read on every platform, so that a wrong note fails where it is written.
#[test]
fn the_windows_note_names_only_the_place_it_installs_into() {
    let note = fs::read_to_string(windows_install_note()).unwrap();
    let commands = note_commands(&note);
    assert_eq!(windows_note_fault(&commands), None);
    let said = note.split_whitespace().collect::<Vec<_>>().join(" ");
    for phrase in [
        format!("{WINDOWS_BIN} on PATH"),
        "`git dupe help` needs the executable alone".to_owned(),
        "Windows has no `man`".to_owned(),
    ] {
        assert!(said.contains(&phrase), "the note does not say {phrase:?}");
    }

    for line in [
        r"New-Item -ItemType Directory -Force ~\bin",
        r"New-Item -ItemType Directory -Force C:\x",
        r"Copy-Item git-dupe.exe ~\bin\",
        r"Copy-Item git-dupe.exe C:\x\",
        r"New-Item -ItemType Directory -Force ~\.local\bin; Remove-Item x",
    ] {
        let mut wrong = commands.clone();
        let at = usize::from(line.starts_with("Copy-Item"));
        wrong[at] = line.to_owned();
        assert!(windows_note_fault(&wrong).is_some(), "{line}");
    }
    let (first, last) = (commands[0].clone(), commands[1].clone());
    for wrong in [vec![], vec![first.clone()], vec![last.clone(), first]] {
        assert!(windows_note_fault(&wrong).is_some(), "{wrong:?}");
    }
}

/// The Windows note followed under PowerShell, from the unpacked directory into a home
/// where `.local` is not there yet. What `man` shows is the one part Windows cannot hold.
fn following_the_windows_note(s: &Scenario, release: &Path, version: &str) {
    let made = release_archive();
    let note = fs::read_to_string(release.join("INSTALL-windows.txt")).unwrap();
    let commands = note_commands(&note);
    assert_eq!(windows_note_fault(&commands), None);
    let home = s.dir().join("home");
    let bin = home.join(".local").join("bin");
    assert!(fs::symlink_metadata(home.join(".local")).is_err());

    // That directory first, and no other git-dupe behind it.
    let path = path_for_an_install(s, &[&bin]);
    let before = s.git(["dupe", "--version"]).variable("PATH", &path).run();
    assert_ne!(before.end, End::Code(0), "{before:?}");
    assert!(
        !before.stdout.starts_with(b"git-dupe version"),
        "{before:?}"
    );

    for command in &commands {
        let words = [
            "-NoProfile",
            "-ExecutionPolicy",
            "Restricted",
            "-Command",
            command,
        ];
        s.program(windows_powershell(), words)
            .from(release)
            .variable("USERPROFILE", &home)
            .variable("HOME", &home)
            .variable("PATH", &path)
            .succeeds();
    }

    let after = s.git(["dupe", "--version"]).variable("PATH", &path).run();
    assert_eq!(after.end, End::Code(0), "{after:?}");
    assert_eq!(after.stdout, version.as_bytes(), "{after:?}");
    let help = s.git(["dupe", "help"]).variable("PATH", &path).succeeds();
    assert!(holds_a_line_of_the_general_text(&help.stdout), "{help:?}");
    for words in [
        &["dupe", "-h"][..],
        &["dupe", "add", "-h"],
        &["dupe", "hide", "-h"],
    ] {
        let asked = s.git(words).variable("PATH", &path).succeeds();
        assert!(!asked.stdout.is_empty(), "{words:?}: {asked:?}");
    }
    s.unavailable("release_archive: the installed page", "what `man` shows");

    assert_eq!(names_in(&bin), ["git-dupe.exe"]);
    assert_eq!(
        fs::read(bin.join("git-dupe.exe")).unwrap(),
        fs::read(&made.executable).unwrap()
    );
}

#[test]
fn following_the_install_note_gives_git_dupe_and_its_page_from_a_temporary_home() {
    let made = release_archive();
    let version = concat!("git-dupe version ", env!("CARGO_PKG_VERSION"), "\n");
    under_each_release(|s| {
        let unpacked = s.dir().join("unpacked");
        fs::create_dir(&unpacked).unwrap();
        made.unpack_into(s, &unpacked);
        let release = unpacked.join(leading_directory());
        if word() == "windows" {
            following_the_windows_note(s, &release, version);
            return;
        }
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

        // That directory first, and no other git-dupe behind it.
        let path = path_for_an_install(s, &[&bin]);

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
