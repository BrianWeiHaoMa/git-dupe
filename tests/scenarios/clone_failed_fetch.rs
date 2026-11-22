//! A `git dupe clone` whose fetch fails (G2, F9, G25): Git's status unchanged, the
//! workspace left attached with what was done, and one `hint:`, after Git's lines and
//! before settle's warnings, saying so and naming the commands that start over, which,
//! run as the hint shows them with a URL that works, end with the private files on disk.
//! No other failure gets that hint: a refusal before any step, a `remote add` that fails,
//! a fetch that worked and a `-b` naming no branch, and a remote with no branch, which
//! keeps its own hint and status 0. The setup recipe of the page's examples, run as a
//! script, starts over only after its own `clone` failed, at its fetch or later, detaching
//! nothing else, and its exit status says whether a private commit is there.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::manpage::{self, Block, Kind};
use crate::harness::{
    End, Output, Scenario, as_git_spells, dupe_clone, holds, names, under_each_release, write,
};

/// The words the hint after a failed fetch holds, and no other line of `clone`'s.
const FETCH_FAILED: &[u8] = b"the fetch failed";

/// No line of `output` is the hint after a failed fetch.
fn no_fetch_hint(output: &Output) {
    assert!(
        output
            .lines("hint")
            .iter()
            .all(|line| !holds(line, FETCH_FAILED)),
        "{output:?}"
    );
}

/// The private files `daily_state` committed on the first machine stand at `dir`.
fn the_files_are_there(dir: &Path) {
    for path in [
        "notes/a.md",
        ".env.local",
        ".vscode/settings.json",
        "docs/notes.md",
    ] {
        assert_eq!(fs::read(dir.join(path)).unwrap(), b"private\n", "{path}");
    }
}

/// The setup recipe of the page's examples, its lines as the page shows them.
fn recipe() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/front/page/20-examples.txt");
    let text = fs::read_to_string(&path).unwrap();
    let kept = manpage::blocks(&text, Kind::Page)
        .into_iter()
        .find_map(|block| match block {
            Block::Kept(lines) if lines.iter().any(|line| line.contains("git dupe clone")) => {
                Some(lines)
            }
            _ => None,
        })
        .expect("the page's setup recipe");
    let indent = kept[0].len() - kept[0].trim_start().len();
    kept.iter()
        .map(|line| format!("{}\n", &line[indent..]))
        .collect()
}

/// Runs the recipe as a script from `dir`, with `url` holding `url`.
fn run_recipe(s: &Scenario, dir: &Path, url: &Path) -> Output {
    let script = s.dir().join("setup.sh");
    fs::write(&script, recipe()).unwrap();
    s.program(s.shell(), [script.as_os_str()])
        .from(dir)
        .variable("url", as_git_spells(url))
        .run()
}

/// Whether the worktree at `dir` is attached, by `In use` 7's test.
fn attached(dir: &Path) -> bool {
    dir.join(".git/dupe").is_dir()
}

#[test]
fn a_failed_fetch_is_named_once_and_the_commands_it_names_start_over() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let second = &m.root;
        // A hidden path a `!` rule exposes, so that settle has a warning to give.
        write(second, ".gitdupe", b"exposed\n");
        let mut ignore = fs::read(second.join(".gitignore")).unwrap();
        ignore.extend_from_slice(b"!exposed\n");
        fs::write(second.join(".gitignore"), ignore).unwrap();
        write(second, "exposed", b"here\n");

        let missing = s.dir().join("nothing-here");
        let failed = dupe_clone(s, second, &[missing.as_os_str()]);
        let gits = s.private(second).git(["fetch", "origin"]).run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");
        assert_eq!(failed.end, gits.end, "{failed:?}");
        assert!(attached(second));
        assert!(!second.join("notes").exists());

        // One hint, right after Git's own lines, then settle's warning.
        let hints: Vec<&[u8]> = failed
            .lines("hint")
            .into_iter()
            .filter(|line| holds(line, FETCH_FAILED))
            .collect();
        assert_eq!(hints.len(), 1, "{failed:?}");
        let hint = hints[0];
        names(hint, b"attached");
        names(hint, b"no private file was written");
        let line = [b"hint: ", hint, b"\n"].concat();
        let at = failed
            .stderr
            .windows(line.len())
            .position(|window| window == line)
            .unwrap();
        assert!(failed.stderr[..at].ends_with(&gits.stderr), "{failed:?}");
        let after = &failed.stderr[at + line.len()..];
        assert!(
            after.starts_with(b"warning: exposed is hidden"),
            "{failed:?}"
        );

        // The commands the hint names, run as it shows them with a URL that works.
        let commands: Vec<Vec<u8>> = hint
            .split(|&byte| byte == b'\'')
            .skip(1)
            .step_by(2)
            .map(<[u8]>::to_vec)
            .collect();
        // `detach`, `detach --force` where it refuses, then `clone`; here `detach` does not
        // refuse, nothing having been fetched.
        let expected: [&[u8]; 3] = [
            b"git dupe detach",
            b"git dupe detach --force",
            b"git dupe clone <url>",
        ];
        assert_eq!(commands, expected);
        // `<url>` stands for the URL, typed as one word: here a variable holding it.
        let good = as_git_spells(&m.first.private_remote);
        for command in [&commands[0], &commands[2]] {
            let command = String::from_utf8(command.clone())
                .unwrap()
                .replace("<url>", "\"$url\"");
            let output = s
                .program(s.shell(), ["-c", &command])
                .from(second)
                .variable("url", &good)
                .run();
            assert_eq!(output.end, End::Code(0), "{command}: {output:?}");
        }
        assert!(attached(second));
        the_files_are_there(second);
    });
}

#[test]
fn no_other_failure_of_clone_gets_the_hint() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let url = m.first.private_remote.as_os_str();

        // Refused before any step: a URL naming the project's repository, and a workspace
        // already attached.
        let public = dupe_clone(s, second, &[first.as_os_str()]);
        assert_eq!(public.end, End::Code(128), "{public:?}");
        no_fetch_hint(&public);
        assert!(!attached(second));
        let done = dupe_clone(s, second, &[url]);
        assert_eq!(done.end, End::Code(0), "{done:?}");
        let again = dupe_clone(s, second, &[url]);
        assert_eq!(again.end, End::Code(128), "{again:?}");
        no_fetch_hint(&again);

        // A fetch that worked, then a `-b` naming no branch: Git's status, attached.
        let other = s.dir().join("other-second");
        s.public_clone(first, &other);
        let missing_branch = [url, OsStr::new("-b"), OsStr::new("missing")];
        let lacking = dupe_clone(s, &other, &missing_branch);
        assert_ne!(lacking.end, End::Code(0), "{lacking:?}");
        assert!(attached(&other));
        no_fetch_hint(&lacking);

        // A `remote add` that fails, on an `origin` a template's configuration brings.
        let template = s.dir().join("template");
        write(
            &template,
            "config",
            b"[remote \"origin\"]\n\turl = elsewhere\n",
        );
        let third = s.dir().join("third-second");
        s.public_clone(first, &third);
        let mut setting = OsString::from("init.templateDir=");
        setting.push(&template);
        let templated = s
            .git([
                OsStr::new("-c"),
                &setting,
                OsStr::new("dupe"),
                OsStr::new("clone"),
                url,
            ])
            .from(&third)
            .run();
        assert_ne!(templated.end, End::Code(0), "{templated:?}");
        assert!(attached(&third));
        no_fetch_hint(&templated);

        // A fetch that worked, then a write step that failed: a filter the project's
        // attributes name for a private file, which the developer's configuration makes
        // required and failing.
        let filtered = s.dir().join("filtered-second");
        s.public_clone(first, &filtered);
        write(&filtered, ".gitattributes", b"notes/a.md filter=broken\n");
        let broken = s
            .git([
                OsStr::new("-c"),
                OsStr::new("filter.broken.smudge=false"),
                OsStr::new("-c"),
                OsStr::new("filter.broken.required=true"),
                OsStr::new("dupe"),
                OsStr::new("clone"),
                url,
            ])
            .from(&filtered)
            .run();
        assert_ne!(broken.end, End::Code(0), "{broken:?}");
        assert!(attached(&filtered));
        s.private(&filtered)
            .git(["rev-parse", "-q", "--verify", "HEAD"])
            .succeeds();
        no_fetch_hint(&broken);

        // A remote with no branch: its own hint, status 0.
        let empty = s.dir().join("empty-private.git");
        s.git([
            OsStr::new("init"),
            OsStr::new("-q"),
            OsStr::new("--bare"),
            empty.as_os_str(),
        ])
        .succeeds();
        let fourth = s.dir().join("fourth-second");
        s.public_clone(first, &fourth);
        let no_branch = dupe_clone(s, &fourth, &[empty.as_os_str()]);
        assert_eq!(no_branch.end, End::Code(0), "{no_branch:?}");
        names(no_branch.only_line("hint"), b"no branch");
        no_fetch_hint(&no_branch);
    });
}

#[test]
fn the_setup_recipe_starts_over_only_after_its_own_clone_failed() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let good = &m.first.private_remote;

        // Its clone fails: it exits non-zero having detached what that clone attached; its
        // next run, with a URL that works, ends with the files, exit 0.
        let missing = s.dir().join("nothing-here");
        let failed = run_recipe(s, second, &missing);
        assert_ne!(failed.end, End::Code(0), "{failed:?}");
        assert!(!attached(second), "{failed:?}");
        let ran = run_recipe(s, second, good);
        assert_eq!(ran.end, End::Code(0), "{ran:?}");
        the_files_are_there(second);
        // Run again where the files are: nothing to do, exit 0.
        let rerun = run_recipe(s, second, good);
        assert_eq!(rerun.end, End::Code(0), "{rerun:?}");

        // Its clone fails after the fetch, at the write step, leaving a private commit
        // and files missing: it removes that attachment all the same, and its next run
        // ends with the files.
        let filtered = s.dir().join("filtered");
        s.public_clone(first, &filtered);
        write(&filtered, ".gitattributes", b"notes/a.md filter=broken\n");
        let script = s.dir().join("setup.sh");
        fs::write(&script, recipe()).unwrap();
        let broken = s
            .program(s.shell(), [script.as_os_str()])
            .from(&filtered)
            .variable("url", as_git_spells(good))
            .variable("GIT_CONFIG_COUNT", "2")
            .variable("GIT_CONFIG_KEY_0", "filter.broken.smudge")
            .variable("GIT_CONFIG_VALUE_0", "false")
            .variable("GIT_CONFIG_KEY_1", "filter.broken.required")
            .variable("GIT_CONFIG_VALUE_1", "true")
            .run();
        assert_ne!(broken.end, End::Code(0), "{broken:?}");
        assert!(!attached(&filtered), "{broken:?}");
        assert!(!filtered.join("notes/a.md").exists());
        let fixed = run_recipe(s, &filtered, good);
        assert_eq!(fixed.end, End::Code(0), "{fixed:?}");
        the_files_are_there(&filtered);

        // A worktree attached by `init`, and one attached to a remote with no branch: it
        // detaches neither, and says by its status that no private commit is there, while
        // `In use` 7's test still answers attached.
        let initialized = s.dir().join("initialized");
        s.public_clone(first, &initialized);
        s.git(["dupe", "init"]).from(&initialized).succeeds();
        let empty: PathBuf = s.dir().join("empty-private.git");
        s.git([
            OsStr::new("init"),
            OsStr::new("-q"),
            OsStr::new("--bare"),
            empty.as_os_str(),
        ])
        .succeeds();
        let no_branch = s.dir().join("no-branch");
        s.public_clone(first, &no_branch);
        for (dir, url) in [(&initialized, good), (&no_branch, &empty)] {
            let output = run_recipe(s, dir, url);
            assert_eq!(output.end, End::Code(1), "{output:?}");
            assert!(attached(dir), "{output:?}");
            let again = run_recipe(s, dir, url);
            assert_eq!(again.end, End::Code(1), "{again:?}");
            assert!(attached(dir), "{again:?}");
        }
    });
}
