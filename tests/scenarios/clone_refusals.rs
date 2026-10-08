//! Where `git dupe clone` attaches nothing (G2, G18, G4, R3, F9): in a workspace already
//! attached, where it settles and changes nothing else; for a word that names a public
//! place; where `init` is refused; outside a repository; and for words that are not
//! `clone URL [-b BRANCH]`, which no Git step follows.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Tree, Worktree, locate_words, names, region, region_rules, run_traced,
    unchanged, under_each_release, usage_line,
};

/// `git dupe clone <words>` from `dir`.
fn clone(s: &Scenario, dir: &Path, words: &[&OsStr]) -> Output {
    let dupe = [OsStr::new("dupe"), OsStr::new("clone")];
    s.git(dupe.iter().chain(words)).from(dir).run()
}

/// The Git steps of `clone`'s own, none of which runs before a refusal.
const STEPS: [&str; 8] = [
    "init",
    "remote",
    "fetch",
    "ls-remote",
    "branch",
    "symbolic-ref",
    "reset",
    "checkout-index",
];

#[test]
fn in_an_attached_workspace_clone_is_refused_and_only_settle_writes() {
    under_each_release(|s| {
        let t = s.pushed_workspace("project");
        let root = &t.root;
        let below = root.join("sub/dir");
        fs::create_dir_all(&below).unwrap();
        let url = t.private_remote.to_str().unwrap();
        // Typed, through a global alias, and through private aliases: one, and a chain of two.
        s.git(["config", "--global", "alias.bring-private", "clone"])
            .succeeds();
        let expansion = format!("clone {url}");
        for (name, value) in [
            ("alias.again", expansion.as_str()),
            ("alias.fetch-home", "clone x"),
            ("alias.go", "fetch-home"),
        ] {
            s.private(root).git(["config", name, value]).succeeds();
        }
        let exclude = root.join(".git/info/exclude");
        let original_exclude = fs::read(&exclude).unwrap();
        let mut expected = region_rules(root);
        expected.push(b"/extra".to_vec());
        expected.sort();
        // A line added to `.gitdupe` by hand since the last command: the refusal's settle
        // brings the region up to date.
        let gitdupe = fs::read(root.join(".gitdupe")).unwrap();
        fs::write(root.join(".gitdupe"), [&gitdupe[..], b"extra\n"].concat()).unwrap();
        let before = [
            Tree::of(root).without(&[&exclude]),
            Tree::of(&t.linked),
            Tree::of(&t.public_remote),
            Tree::of(&t.private_remote),
        ];
        let log = s.dir().join("trace");

        for words in [
            &["clone", url][..],
            &["clone", url, "-b", "main"],
            &["bring-private", url],
            &["again"],
            &["go"],
        ] {
            for directory in [root, &below] {
                fs::write(&exclude, &original_exclude).unwrap();
                let (refusal, runs) =
                    run_traced(s.git(["dupe"].iter().chain(words)).from(directory), &log);
                assert_eq!(refusal.end, End::Code(128), "{words:?}: {refusal:?}");
                assert!(refusal.stdout.is_empty(), "{words:?}: {refusal:?}");
                let line = refusal.only_line("fatal");
                names(line, b"already attached");
                // The commands `help clone` names for starting over.
                names(line, b"'git dupe detach'");
                names(line, b"'git dupe detach --force'");
                names(line, b"'git dupe clone'");
                let own = runs.own();
                for step in STEPS {
                    assert_eq!(own.of(step), 0, "{words:?}: {step}: {runs:?}");
                }
                assert_eq!(region_rules(root), expected, "{words:?}");
                let after = [
                    Tree::of(root).without(&[&exclude]),
                    Tree::of(&t.linked),
                    Tree::of(&t.public_remote),
                    Tree::of(&t.private_remote),
                ];
                for (was, is) in before.iter().zip(&after) {
                    let changed = was.changed_in(is);
                    assert!(changed.is_empty(), "{words:?} changed: {changed:?}");
                }
            }
        }
    });
}

#[test]
fn a_word_naming_a_public_place_is_refused_before_anything_is_created() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let second = &m.root;
        let linked = s.dir().join("project-second-linked");
        s.linked_worktree(second, &linked);
        let below = second.join("docs");
        let url = m.first.private_remote.as_os_str();
        // The second machine's public remote `origin` is the first machine's root.
        let public = m.first.root.as_os_str();
        let file_url = [&b"file://"[..], public.as_bytes()].concat();
        let assigned = [&b"x="[..], second.as_os_str().as_bytes()].concat();
        let cases: [(&[&OsStr], &OsStr); 7] = [
            (&[public], public),
            (
                &[OsStr::from_bytes(&file_url)],
                OsStr::from_bytes(&file_url),
            ),
            (&[OsStr::new(".")], OsStr::new(".")),
            (&[second.as_os_str()], second.as_os_str()),
            (&[linked.as_os_str()], linked.as_os_str()),
            (
                &[OsStr::from_bytes(&assigned)],
                OsStr::from_bytes(&assigned),
            ),
            (&[url, OsStr::new("-b"), OsStr::new(".")], OsStr::new(".")),
        ];
        let before = Tree::of(second);
        for (words, word) in cases {
            for directory in [second, &below] {
                let refusal = clone(s, directory, words);
                assert_eq!(refusal.end, End::Code(128), "{words:?}: {refusal:?}");
                assert!(refusal.stdout.is_empty(), "{refusal:?}");
                let line = refusal.only_line("fatal");
                names(line, &[b"'", word.as_bytes(), b"' names "].concat());
                names(line, b"git dupe git");
                unchanged(&before, second);
            }
        }
        // The same words with the private URL where the public one stood attach.
        let attached = clone(s, second, &[url, OsStr::new("-b"), OsStr::new("main")]);
        assert_eq!(attached.end, End::Code(0), "{attached:?}");
        assert!(second.join("notes/a.md").is_file());
    });
}

/// What `git dupe <command> <url>` and `git dupe init` print and end with from `from`,
/// where nothing below `dir` may change.
fn as_init_ends(s: &Scenario, from: &Path, dir: &Path, url: &OsStr) -> Output {
    let before = Tree::of(dir);
    let init = s.git(["dupe", "init"]).from(from).run();
    unchanged(&before, dir);
    let cloned = clone(s, from, &[url]);
    unchanged(&before, dir);
    assert_eq!(cloned, init, "{}", from.display());
    cloned
}

#[test]
fn where_init_is_refused_clone_is_refused_alike_and_creates_nothing() {
    under_each_release(|s| {
        let empty = s.dir().join("private.git");
        s.bare_repository(&empty);
        let url = empty.as_os_str();
        let repository = |name: &str| -> PathBuf {
            let dir = s.dir().join(name);
            s.repository(&dir);
            dir
        };

        // A linked worktree whose Git directory was moved by hand out of `worktrees`, so
        // that no `git worktree add` made it, naming that directory; a linked worktree
        // `git worktree add` made beside it attaches from the same URL.
        let main = repository("main");
        let moved = s.dir().join("moved");
        s.linked_worktree(&main, &moved);
        let admin = main.join(".git/elsewhere/moved");
        fs::create_dir(main.join(".git/elsewhere")).unwrap();
        fs::rename(main.join(".git/worktrees/moved"), &admin).unwrap();
        fs::write(
            moved.join(".git"),
            [b"gitdir: ", admin.as_os_str().as_bytes(), b"\n"].concat(),
        )
        .unwrap();
        let refused = as_init_ends(s, &moved, &main.join(".git"), url);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        names(refused.only_line("fatal"), admin.as_os_str().as_bytes());
        let linked = s.dir().join("linked");
        s.linked_worktree(&main, &linked);
        let attached = clone(s, &linked, &[url]);
        assert_eq!(attached.end, End::Code(0), "{attached:?}");
        assert!(
            Worktree::read(s, &linked)
                .private_directory()
                .join("HEAD")
                .is_file()
        );
        assert!(!main.join(".git/dupe").exists());

        // A submodule checkout.
        let public = repository("public");
        let superproject = repository("super");
        let added = s
            .git([
                OsStr::new("-c"),
                OsStr::new("protocol.file.allow=always"),
                OsStr::new("submodule"),
                OsStr::new("add"),
                public.as_os_str(),
                OsStr::new("sub"),
            ])
            .from(&superproject)
            .run();
        assert_eq!(added.end, End::Code(0), "{added:?}");
        let refused = as_init_ends(s, &superproject.join("sub"), &superproject, url);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        refused.only_line("fatal");
        s.git(["config", "--global", "alias.bring-private", "clone"])
            .succeeds();
        let before = Tree::of(&superproject);
        let aliased = s
            .git([OsStr::new("dupe"), OsStr::new("bring-private"), url])
            .from(&superproject.join("sub"))
            .run();
        assert_eq!(aliased, refused);
        unchanged(&before, &superproject);

        // A project that tracks `.gitdupe`.
        let tracked = repository("tracked");
        fs::write(tracked.join(".gitdupe"), b"notes\n").unwrap();
        s.git(["add", "-f", "--", ".gitdupe"])
            .from(&tracked)
            .succeeds();
        let refused = as_init_ends(s, &tracked, &tracked, url);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        names(refused.only_line("fatal"), b".gitdupe");

        // A `.git` at the root that is not the repository's own directory.
        let linking = repository("linking");
        let admin = s.dir().join("linking-admin");
        fs::rename(linking.join(".git"), &admin).unwrap();
        symlink(&admin, linking.join(".git")).unwrap();
        let refused = as_init_ends(s, &linking, &admin, url);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        names(refused.only_line("fatal"), b".git");

        // A bare repository and outside any repository: Git's own end, as for `init`.
        let bare = s.dir().join("bare.git");
        s.bare_repository(&bare);
        let outside = s.dir().join("outside");
        fs::create_dir(&outside).unwrap();
        for place in [&bare, &outside] {
            let gits = s.git(locate_words(true)).from(place).run();
            let ended = as_init_ends(s, place, place, url);
            assert_eq!(ended.end, gits.end, "{ended:?}");
            assert_eq!(ended.stderr, gits.stderr, "{ended:?}");
            assert!(ended.stdout.is_empty(), "{ended:?}");
        }
    });
}

#[test]
fn words_that_are_not_clone_url_b_branch_are_a_usage_error_after_the_locate_alone() {
    under_each_release(|s| {
        let text = s.git(["dupe", "help", "clone"]).succeeds().stdout;
        let misuses: [&[&str]; 6] = [
            &[],
            &["x", "extra"],
            &["--depth", "1", "x"],
            &["-b"],
            &["--branch=main", "x"],
            &["-bmain", "x"],
        ];
        let attached = s.dir().join("attached");
        let unattached = s.dir().join("unattached");
        s.attached_project(&attached);
        s.repository(&unattached);
        // A region gone stale: a settle would write it.
        fs::write(attached.join(".gitdupe"), b"notes\n").unwrap();
        let log = s.dir().join("trace");
        for dir in [&attached, &unattached] {
            let below = dir.join("sub");
            fs::create_dir_all(&below).unwrap();
            let before = Tree::of(dir);
            for words in misuses {
                for from in [dir, &below] {
                    let (output, runs) = run_traced(
                        s.git(["dupe", "clone"].iter().chain(words)).from(from),
                        &log,
                    );
                    assert_eq!(output.end, End::Code(129), "{words:?}: {output:?}");
                    assert!(output.stdout.is_empty(), "{output:?}");
                    output.line_then("error", usage_line(&text));
                    // The locate run, and nothing after it: no step and no settle.
                    assert_eq!(
                        runs.own().commands(),
                        [&b"rev-parse"[..]],
                        "{words:?}: {runs:?}"
                    );
                    unchanged(&before, dir);
                }
            }
        }
        assert_eq!(region(&attached).unwrap().rules, [b"/.gitdupe"]);

        // Outside any repository the same words end as the locate run ends there.
        let gits = s.git(locate_words(true)).run();
        assert_eq!(gits.end, End::Code(128), "{gits:?}");
        for words in misuses {
            let output = s.git(["dupe", "clone"].iter().chain(words)).run();
            assert_eq!(output.end, gits.end, "{words:?}: {output:?}");
            assert_eq!(output.stderr, gits.stderr, "{words:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
        }
    });
}
