//! Another worktree's region hides files here without adopting them (G27, In use 11).
//! Git decides exposure; a shared exclude file is only maintained where G6 allows it.

use crate::harness::{End, Output, Scenario, Tree, Worktree, offered, under_each_release, write};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, symlink};

fn main_worktree(s: &Scenario, name: &str) -> Worktree {
    let root = s.dir().join(name);
    s.attached_project(&root);
    write(&root, ".gitdupe", b"");
    Worktree::read(s, &root)
}

fn linked(s: &Scenario, main: &Worktree, name: &OsStr) -> Worktree {
    let root = main.root.parent().unwrap().join(name);
    s.linked_worktree(&main.root, &root);
    s.init(&root);
    write(&root, ".gitdupe", b"");
    Worktree::read(s, &root)
}

fn hide(s: &Scenario, wt: &Worktree, path: &str) {
    s.git(["dupe", "hide", "--", path])
        .from(&wt.root)
        .succeeds();
    assert!(
        wt.region()
            .unwrap()
            .rules
            .contains(&format!("/{path}").into_bytes())
    );
}

fn foreign(path: &str, owners: &[u8], hideable: bool) -> Vec<u8> {
    let mut line = [
        path.as_bytes(),
        b" stands here and is hidden by ",
        owners,
        b" alone: public Git ignores it here, and this worktree does not hide it",
    ]
    .concat();
    if hideable {
        line.extend_from_slice(
            format!("; run from the root, 'git dupe hide -- {path}' hides it here too").as_bytes(),
        );
    }
    line
}

fn warnings(output: &Output, expected: &[Vec<u8>]) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert_eq!(
        output.lines("warning"),
        expected.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        "{output:?}"
    );
    for level in ["fatal", "error"] {
        assert!(output.lines(level).is_empty(), "{output:?}");
    }
}

fn public_status(s: &Scenario, wt: &Worktree, expected: &[u8]) {
    let status = s
        .git(["status", "--porcelain", "--untracked-files=all"])
        .from(&wt.root)
        .succeeds();
    assert_eq!(status.stdout, expected, "{status:?}");
}

/// An unchanged command warns without adopting files, staging, or rewriting any region.
/// The public listing used for the snapshot does not settle or change a region.
fn steady(s: &Scenario, wt: &Worktree, expected: &[Vec<u8>]) -> Output {
    let working = Tree::working(&wt.root);
    let gitdupe = fs::read(wt.root.join(".gitdupe")).unwrap();
    let private_index = wt
        .private(s)
        .git(["ls-files", "-s", "-z"])
        .succeeds()
        .stdout;
    let public_index = s
        .git(["ls-files", "-s", "-z"])
        .from(&wt.root)
        .succeeds()
        .stdout;
    let exclude = wt.common_directory.join("info/exclude");
    let bytes = fs::read(&exclude).unwrap();
    let inode = fs::metadata(&exclude).unwrap().ino();
    let output = s
        .git(["dupe", "status", "--porcelain"])
        .from(&wt.root)
        .succeeds();
    warnings(&output, expected);
    let index = s
        .git(["dupe", "ls-files", "-s", "-z"])
        .from(&wt.root)
        .succeeds();
    warnings(&index, expected);
    assert_eq!(index.stdout, private_index, "{index:?}");
    assert_eq!(fs::read(wt.root.join(".gitdupe")).unwrap(), gitdupe);
    assert!(working.changed_in(&Tree::working(&wt.root)).is_empty());
    assert_eq!(
        s.git(["ls-files", "-s", "-z"])
            .from(&wt.root)
            .succeeds()
            .stdout,
        public_index
    );
    assert_eq!(fs::read(&exclude).unwrap(), bytes);
    assert_eq!(fs::metadata(&exclude).unwrap().ino(), inode);
    output
}

#[test]
fn foreign_warning_repeats_without_adoption_until_added_here() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "notes");
        write(&wt.root, "notes/x.md", b"local note\n");
        public_status(s, &wt, b"");
        let expected = [foreign("notes", b"the main worktree", true)];
        let first = steady(s, &wt, &expected);
        let second = steady(s, &wt, &expected);
        assert_eq!(first, second);
        assert_eq!(fs::read(wt.root.join(".gitdupe")).unwrap(), b"");
        assert_eq!(wt.region().unwrap().rules, [b"/.gitdupe"]);
        s.git(["dupe", "add", "notes/"]).from(&wt.root).succeeds();
        steady(s, &wt, &[]);
        assert_eq!(
            wt.private(s)
                .git(["ls-files", "--", "notes"])
                .succeeds()
                .stdout,
            b"notes/x.md\n"
        );
        assert_eq!(
            fs::read(wt.root.join("notes/x.md")).unwrap(),
            b"local note\n"
        );
        public_status(s, &wt, b"");
    });
}

#[test]
fn foreign_scratch_story_stages_the_previously_invisible_file() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "scratch");
        write(&wt.root, "scratch/a.py", b"print('local')\n");
        public_status(s, &wt, b"");
        steady(s, &wt, &[foreign("scratch", b"the main worktree", true)]);
        let main_region = main.region_bytes();
        let added = s.git(["dupe", "add", "scratch/"]).from(&wt.root).succeeds();
        warnings(&added, &[]);
        let listed = s
            .git(["dupe", "ls-files", "-s", "--", "scratch/a.py"])
            .from(&wt.root)
            .succeeds();
        warnings(&listed, &[]);
        assert_eq!(
            listed.stdout,
            wt.private(s)
                .git(["ls-files", "-s", "--", "scratch/a.py"])
                .succeeds()
                .stdout
        );
        assert!(!listed.stdout.is_empty());
        assert_eq!(
            wt.private(s)
                .git(["cat-file", "blob", ":scratch/a.py"])
                .succeeds()
                .stdout,
            b"print('local')\n"
        );
        steady(s, &wt, &[]);
        assert_eq!(main.region_bytes(), main_region);
    });
}

#[test]
fn foreign_owners_use_whole_worktree_names_and_preserve_non_utf8_bytes() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let agent = linked(s, &main, OsStr::new("agent"));
        let agent2 = linked(s, &main, OsStr::new("agent2"));
        let bytes = linked(s, &main, OsStr::from_bytes(b"caf\xe9"));
        assert_eq!(agent.name(), Some(OsStr::new("agent")));
        assert_eq!(agent2.name(), Some(OsStr::new("agent2")));
        assert_eq!(bytes.name(), Some(OsStr::from_bytes(b"caf\xe9")));
        hide(s, &agent2, "scratch");
        hide(s, &bytes, "notes");
        write(&main.root, "scratch/a.py", b"scratch\n");
        write(&main.root, "notes/x.md", b"notes\n");
        steady(
            s,
            &main,
            &[
                foreign("notes", b"worktree caf\xe9", true),
                foreign("scratch", b"worktree agent2", true),
            ],
        );
        public_status(s, &main, b"");
        assert_eq!(agent.region().unwrap().rules, [b"/.gitdupe"]);
    });
}

#[test]
fn foreign_path_has_one_warning_naming_both_owners() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let owner = linked(s, &main, OsStr::new("owner"));
        let wt = linked(s, &main, OsStr::new("observer"));
        hide(s, &main, "notes");
        hide(s, &owner, "notes");
        write(&wt.root, "notes/x.md", b"here\n");
        steady(
            s,
            &wt,
            &[foreign(
                "notes",
                b"the main worktree and worktree owner",
                true,
            )],
        );
        public_status(s, &wt, b"");
    });
}

#[test]
fn foreign_warning_is_suppressed_only_by_hiding_the_path_or_an_ancestor_here() {
    under_each_release(|s| {
        for (name, local) in [
            ("equal", "notes/sub"),
            ("ancestor", "notes"),
            ("child", "notes/sub/child"),
        ] {
            let main = main_worktree(s, name);
            let wt = linked(s, &main, OsStr::new(&format!("{name}-agent")));
            hide(s, &main, "notes/sub");
            hide(s, &wt, local);
            write(&wt.root, "notes/sub/child/x.md", b"here\n");
            let expected = if name == "child" {
                vec![foreign("notes/sub", b"the main worktree", true)]
            } else {
                vec![]
            };
            steady(s, &wt, &expected);
            public_status(s, &wt, b"");
        }
        let main = main_worktree(s, "tracked-child");
        let wt = linked(s, &main, OsStr::new("tracked-agent"));
        hide(s, &main, "notes");
        write(&wt.root, "notes/x.md", b"private child\n");
        s.git(["dupe", "add", "notes/x.md"])
            .from(&wt.root)
            .succeeds();
        steady(s, &wt, &[foreign("notes", b"the main worktree", true)]);
        assert_eq!(
            wt.private(s)
                .git(["ls-files", "--", "notes"])
                .succeeds()
                .stdout,
            b"notes/x.md\n"
        );
    });
}

#[test]
fn foreign_warning_ends_when_the_owner_releases_or_the_local_path_is_removed() {
    under_each_release(|s| {
        for name in ["release", "remove"] {
            let main = main_worktree(s, name);
            let wt = linked(s, &main, OsStr::new(&format!("{name}-agent")));
            hide(s, &main, "notes");
            write(&wt.root, "notes/x.md", b"here\n");
            steady(s, &wt, &[foreign("notes", b"the main worktree", true)]);
            if name == "release" {
                s.git(["dupe", "unhide", "notes"])
                    .from(&main.root)
                    .succeeds();
                public_status(s, &wt, b"?? notes/x.md\n");
            } else {
                fs::remove_dir_all(wt.root.join("notes")).unwrap();
                public_status(s, &wt, b"");
            }
            steady(s, &wt, &[]);
        }
    });
}

#[test]
fn foreign_absent_path_is_not_named() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "notes");
        assert!(!wt.root.join("notes").exists());
        steady(s, &wt, &[]);
        public_status(s, &wt, b"");
    });
}

#[test]
fn foreign_exact_public_tracking_on_the_linked_branch_suppresses_the_warning() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "notes");
        s.git(["switch", "-q", "-c", "agent"])
            .from(&wt.root)
            .succeeds();
        write(&wt.root, "notes", b"public on agent\n");
        s.git(["add", "-f", "--", "notes"])
            .from(&wt.root)
            .succeeds();
        s.commit_public(&wt.root);
        assert_eq!(
            s.git(["ls-files", "--", "notes"])
                .from(&wt.root)
                .succeeds()
                .stdout,
            b"notes\n"
        );
        assert!(
            s.git(["ls-files", "--", "notes"])
                .from(&main.root)
                .succeeds()
                .stdout
                .is_empty()
        );
        steady(s, &wt, &[]);
        public_status(s, &wt, b"");
    });
}

#[test]
fn foreign_directory_with_a_publicly_tracked_child_still_warns_without_a_hide_remedy() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "notes");
        write(&wt.root, "notes/public.md", b"public\n");
        write(&wt.root, "notes/local.md", b"local\n");
        s.git(["add", "-f", "--", "notes/public.md"])
            .from(&wt.root)
            .succeeds();
        s.commit_public(&wt.root);
        steady(s, &wt, &[foreign("notes", b"the main worktree", false)]);
        public_status(s, &wt, b"");
        assert_eq!(
            s.git(["ls-files", "--", "notes"])
                .from(&wt.root)
                .succeeds()
                .stdout,
            b"notes/public.md\n"
        );
    });
}

#[test]
fn foreign_reinclusion_gets_neither_foreign_nor_hidden_exposure_warning() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "notes");
        write(&wt.root, "notes/x.md", b"here\n");
        write(&wt.root, ".gitignore", b"!/notes\n");
        steady(s, &wt, &[]);
        public_status(s, &wt, b" M .gitignore\n?? notes/x.md\n");
    });
}

#[test]
fn foreign_user_ignore_rule_still_names_the_region_owner() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "notes");
        write(&wt.root, "notes/x.md", b"here\n");
        write(&wt.root, ".gitignore", b"/notes\n");
        assert_eq!(
            s.git(["check-ignore", "-v", "notes"])
                .from(&wt.root)
                .succeeds()
                .stdout,
            b".gitignore:1:/notes\tnotes\n"
        );
        steady(s, &wt, &[foreign("notes", b"the main worktree", true)]);
        public_status(s, &wt, b" M .gitignore\n");
    });
}

#[test]
fn foreign_warning_coexists_with_hidden_dual_tracked_and_released_warnings() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        for path in [
            "absent",
            "directory",
            "exact",
            "notes",
            "reincluded",
            "user",
        ] {
            hide(s, &main, path);
        }
        hide(s, &wt, "exposed");
        hide(s, &wt, "released");
        for path in [
            "both",
            "exposed",
            "released",
            "notes/x.md",
            "exact",
            "directory/public.md",
            "directory/local.md",
            "reincluded/x.md",
            "user/x.md",
        ] {
            write(&wt.root, path, b"here\n");
        }
        s.git(["dupe", "add", "both"]).from(&wt.root).succeeds();
        s.git(["add", "-f", "--", "both", "exact", "directory/public.md"])
            .from(&wt.root)
            .succeeds();
        s.commit_public(&wt.root);
        write(&wt.root, ".gitignore", b"!/exposed\n!/reincluded\n/user\n");
        let main_region = main.region_bytes();
        let index = wt
            .private(s)
            .git(["ls-files", "-s", "-z"])
            .succeeds()
            .stdout;
        write(&wt.root, ".gitdupe", b"exposed\n");
        let working = Tree::working(&wt.root);
        let output = s
            .git(["dupe", "status", "--porcelain"])
            .from(&wt.root)
            .succeeds();
        warnings(&output, &[
            b"exposed is hidden but public Git does not ignore it: .gitignore:1 re-includes it".to_vec(),
            b"released is no longer hidden and is visible to public Git; run from the root, 'git dupe hide -- released' hides it again".to_vec(),
            foreign("directory", b"the main worktree", false),
            foreign("notes", b"the main worktree", true),
            foreign("user", b"the main worktree", true),
            b"both is tracked by both the public and the private repository; 'git rm --cached' or 'git dupe rm --cached' releases it from one".to_vec(),
        ]);
        assert_eq!(
            wt.private(s)
                .git(["ls-files", "-s", "-z"])
                .succeeds()
                .stdout,
            index
        );
        assert!(working.changed_in(&Tree::working(&wt.root)).is_empty());
        assert_eq!(main.region_bytes(), main_region);
        assert_eq!(
            wt.region().unwrap().rules,
            [
                b"/.gitdupe".to_vec(),
                b"/both".to_vec(),
                b"/exposed".to_vec()
            ]
        );
        public_status(
            s,
            &wt,
            b" M .gitignore\n?? exposed\n?? reincluded/x.md\n?? released\n",
        );
    });
}

#[test]
fn foreign_owner_is_named_when_a_path_is_released_here_but_remains_ignored() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "notes");
        hide(s, &wt, "notes");
        write(&wt.root, "notes/x.md", b"here\n");
        let main_region = main.region_bytes();
        let index = wt
            .private(s)
            .git(["ls-files", "-s", "-z", "--", "notes"])
            .succeeds()
            .stdout;
        let output = s.git(["dupe", "unhide", "notes"]).from(&wt.root).succeeds();
        warnings(&output, &[foreign("notes", b"the main worktree", true)]);
        assert_eq!(main.region_bytes(), main_region);
        assert_eq!(wt.region().unwrap().rules, [b"/.gitdupe"]);
        assert_eq!(fs::read(wt.root.join(".gitdupe")).unwrap(), b"");
        assert_eq!(
            wt.private(s)
                .git(["ls-files", "-s", "-z", "--", "notes"])
                .succeeds()
                .stdout,
            index
        );
        assert_eq!(fs::read(wt.root.join("notes/x.md")).unwrap(), b"here\n");
        steady(s, &wt, &[foreign("notes", b"the main worktree", true)]);
        public_status(s, &wt, b"");
    });
}

#[test]
fn foreign_overlap_reincluded_here_is_released_with_one_visible_warning() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "notes");
        hide(s, &wt, "notes");
        write(&wt.root, "notes/x.md", b"untracked privately\n");
        write(&wt.root, ".gitignore", b"!/notes\n");
        let main_region = main.region_bytes();
        let output = s.git(["dupe", "unhide", "notes"]).from(&wt.root).succeeds();
        warnings(&output, &[b"notes is no longer hidden and is visible to public Git; run from the root, 'git dupe hide -- notes' hides it again".to_vec()]);
        assert_eq!(main.region_bytes(), main_region);
        assert_eq!(wt.region().unwrap().rules, [b"/.gitdupe"]);
        assert_eq!(fs::read(wt.root.join(".gitdupe")).unwrap(), b"");
        assert!(
            wt.private(s)
                .git(["ls-files", "--", "notes"])
                .succeeds()
                .stdout
                .is_empty()
        );
        assert_eq!(
            fs::read(wt.root.join("notes/x.md")).unwrap(),
            b"untracked privately\n"
        );
        steady(s, &wt, &[]);
        public_status(s, &wt, b" M .gitignore\n?? notes/x.md\n");
    });
}

#[test]
fn foreign_path_beyond_a_link_is_unnamed_while_local_exposure_is_checked() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "link/child");
        hide(s, &wt, "exposed");
        write(&main.root, "target/child", b"beyond the link\n");
        symlink(main.root.join("target"), wt.root.join("link")).unwrap();
        write(&wt.root, "exposed", b"here\n");
        write(&wt.root, ".gitignore", b"!/exposed\n");
        steady(
            s,
            &wt,
            &[
                b"exposed is hidden but public Git does not ignore it: .gitignore:1 re-includes it"
                    .to_vec(),
            ],
        );
        assert_eq!(
            fs::read(main.root.join("target/child")).unwrap(),
            b"beyond the link\n"
        );
        public_status(s, &wt, b" M .gitignore\n?? exposed\n?? link\n");
    });
}

#[test]
fn foreign_links_at_the_path_itself_are_named_even_when_dangling() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        hide(s, &main, "link");
        hide(s, &main, "dangling");
        symlink(wt.root.join("docs"), wt.root.join("link")).unwrap();
        symlink("nothing", wt.root.join("dangling")).unwrap();
        steady(
            s,
            &wt,
            &[
                foreign("dangling", b"the main worktree", true),
                foreign("link", b"the main worktree", true),
            ],
        );
        public_status(s, &wt, b"");
    });
}

#[test]
fn foreign_paths_are_not_named_when_the_region_cannot_be_maintained() {
    under_each_release(|s| {
        for name in ["info-link", "exclude-link"] {
            let main = main_worktree(s, name);
            let wt = linked(s, &main, OsStr::new(&format!("{name}-agent")));
            hide(s, &main, "notes");
            write(&wt.root, "notes/x.md", b"here\n");
            let info = main.common_directory.join("info");
            let exclude = info.join("exclude");
            let saved = fs::read(&exclude).unwrap();
            let (link, target, cause) = if name == "info-link" {
                let target = main.common_directory.join("saved-info");
                fs::rename(&info, &target).unwrap();
                symlink(&target, &info).unwrap();
                (info, target, "is a symbolic link")
            } else {
                fs::remove_file(&exclude).unwrap();
                let target = main.common_directory.join("absent-exclude");
                symlink(&target, &exclude).unwrap();
                (exclude.clone(), target, "is a symbolic link to nothing")
            };
            let working = Tree::working(&wt.root);
            let private_index = wt
                .private(s)
                .git(["ls-files", "-s", "-z"])
                .succeeds()
                .stdout;
            let public_index = s
                .git(["ls-files", "-s", "-z"])
                .from(&wt.root)
                .succeeds()
                .stdout;
            let inode = fs::symlink_metadata(&link).unwrap().ino();
            let output = s
                .git(["dupe", "status", "--porcelain"])
                .from(&wt.root)
                .succeeds();
            let mut expected = vec![format!("the managed region cannot be maintained: {} {cause}; private files may be visible to public Git", link.display()).into_bytes()];
            if name == "exclude-link" {
                expected.push(b".gitdupe is hidden but public Git does not ignore it".to_vec());
            }
            warnings(&output, &expected);
            // G6 keeps a failing handler's status too, without foreign attribution.
            let direct = wt.private(s).git(["config", "--get", "absent.key"]).run();
            assert_eq!(direct.end, End::Code(1), "{direct:?}");
            let failed = s
                .git(["dupe", "git", "config", "--get", "absent.key"])
                .from(&wt.root)
                .run();
            assert_eq!(failed.end, direct.end, "{failed:?}");
            assert_eq!(failed.stdout, direct.stdout, "{failed:?}");
            let expected_stderr: Vec<u8> = expected
                .iter()
                .flat_map(|line| [b"warning: ", line.as_slice(), b"\n"].concat())
                .collect();
            assert_eq!(failed.stderr, expected_stderr, "{failed:?}");
            assert!(working.changed_in(&Tree::working(&wt.root)).is_empty());
            assert_eq!(
                wt.private(s)
                    .git(["ls-files", "-s", "-z"])
                    .succeeds()
                    .stdout,
                private_index
            );
            assert_eq!(
                s.git(["ls-files", "-s", "-z"])
                    .from(&wt.root)
                    .succeeds()
                    .stdout,
                public_index
            );
            assert_eq!(fs::read_link(&link).unwrap(), target);
            assert_eq!(fs::symlink_metadata(&link).unwrap().ino(), inode);
            if name == "info-link" {
                assert_eq!(fs::read(&exclude).unwrap(), saved);
                public_status(s, &wt, b"");
            } else {
                assert!(!target.exists());
                public_status(s, &wt, b"?? .gitdupe\n?? notes/x.md\n");
            }
        }
    });
}

#[test]
fn foreign_and_released_hide_remedies_take_their_path_whole() {
    under_each_release(|s| {
        let main = main_worktree(s, "project");
        let wt = linked(s, &main, OsStr::new("agent"));
        // Quotes and `$` a shell reads, and a leading `:` that `hide` reads as magic
        // unless it is typed `./:colon`.
        let paths = [":colon", "it's $HOME", "two words"];
        for path in paths {
            s.git(["dupe", "hide", "--", &format!("./{path}")])
                .from(&main.root)
                .succeeds();
            write(&wt.root, path, b"here\n");
        }
        let output = s.git(["dupe", "status"]).from(&wt.root).succeeds();
        let lines = output.lines("warning");
        assert_eq!(lines.len(), paths.len(), "{output:?}");
        for line in lines {
            // The command as the line shows it, run by a shell as typed (G25).
            let command = offered(line, b"run from the root, '", b"' hides it here too");
            let shell = std::str::from_utf8(command).unwrap();
            s.program("/bin/sh", ["-c", shell])
                .from(&wt.root)
                .succeeds();
        }
        assert_eq!(
            fs::read(wt.root.join(".gitdupe")).unwrap(),
            b":colon\nit's $HOME\ntwo words\n"
        );
        let settled = s.git(["dupe", "status"]).from(&wt.root).succeeds();
        assert!(settled.lines("warning").is_empty(), "{settled:?}");

        // A released path's own remedy, for a path no other worktree hides, takes it
        // whole the same way (G9).
        write(&wt.root, "own words", b"here\n");
        s.git(["dupe", "hide", "--", "own words"])
            .from(&wt.root)
            .succeeds();
        let unhidden = s
            .git(["dupe", "unhide", "--", "own words"])
            .from(&wt.root)
            .succeeds();
        let released = unhidden.lines("warning");
        assert_eq!(released.len(), 1, "{unhidden:?}");
        assert!(
            released[0].starts_with(b"own words is no longer hidden and is visible to public Git"),
            "{unhidden:?}"
        );
        let command = offered(released[0], b"run from the root, '", b"' hides it again");
        s.program("/bin/sh", ["-c", std::str::from_utf8(command).unwrap()])
            .from(&wt.root)
            .succeeds();
        assert_eq!(
            fs::read(wt.root.join(".gitdupe")).unwrap(),
            b":colon\nit's $HOME\ntwo words\nown words\n"
        );
        let settled = s.git(["dupe", "status"]).from(&wt.root).succeeds();
        assert!(settled.lines("warning").is_empty(), "{settled:?}");
    });
}
