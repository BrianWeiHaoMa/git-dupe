//! Detaching a linked worktree removes its region and repository alone (G3, G28),
//! including refusals, failed replacement, and interrupted removal. Fixtures are made
//! through Git afresh: copying a linked root would retain its original Git directory.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use crate::harness::{
    End, Output, Point, Scenario, Tree, Worktree, holds, names, now_visible, unchanged,
    under_each_release, write,
};

struct Attached {
    main: Worktree,
    linked: Worktree,
    main_private: Tree,
    main_region: Vec<u8>,
}

fn commit(s: &Scenario, root: &Path) {
    s.git([
        "-c",
        "maintenance.auto=false",
        "-c",
        "user.name=Scenario",
        "-c",
        "user.email=scenario@example.invalid",
        "dupe",
        "commit",
        "-qm",
        "private files",
    ])
    .from(root)
    .succeeds();
}

fn attached(s: &Scenario, name: &str) -> Attached {
    let main = s.dir().join(format!("{name}-main"));
    s.attached_project(&main);
    write(&main, "notes/main.md", b"main private\n");
    s.git(["dupe", "hide", "notes"]).from(&main).succeeds();
    s.git(["dupe", "add", "notes"]).from(&main).succeeds();
    commit(s, &main);

    let linked = s.dir().join(format!("{name}-linked"));
    s.linked_worktree(&main, &linked);
    s.init(&linked);
    write(&linked, "notes/x.md", b"linked private\n");
    write(&linked, "personal/todo.md", b"only linked\n");
    s.git(["dupe", "hide", "notes", "personal"])
        .from(&linked)
        .succeeds();
    s.git(["dupe", "add", "notes", "personal"])
        .from(&linked)
        .succeeds();
    commit(s, &linked);
    let main = Worktree::read(s, &main);
    let linked = Worktree::read(s, &linked);
    assert!(main.private_directory().is_dir());
    assert!(linked.private_directory().is_dir());
    assert!(linked.region().is_some());
    Attached {
        main_private: Tree::of(&main.private_directory()),
        main_region: main.region_bytes(),
        main,
        linked,
    }
}

impl Attached {
    fn sibling_unchanged(&self) {
        assert!(self.main.private_directory().is_dir());
        unchanged(&self.main_private, &self.main.private_directory());
        assert_eq!(self.main.region_bytes(), self.main_region);
    }

    fn detached(&self) {
        assert_eq!(self.linked.region(), None);
        assert!(!self.linked.private_directory().exists());
        self.sibling_unchanged();
    }
}

fn pushed(s: &Scenario, fixture: &Attached, name: &str) {
    let remote = s.dir().join(format!("{name}-remote.git"));
    s.bare_repository(&remote);
    s.git([
        OsStr::new("dupe"),
        OsStr::new("remote"),
        OsStr::new("add"),
        OsStr::new("origin"),
        remote.as_os_str(),
    ])
    .from(&fixture.linked.root)
    .succeeds();
    let branch = fixture
        .linked
        .private(s)
        .git(["symbolic-ref", "--short", "HEAD"])
        .succeeds();
    s.git([
        OsStr::new("dupe"),
        OsStr::new("push"),
        OsStr::new("-u"),
        OsStr::new("origin"),
        OsStr::new(std::str::from_utf8(branch.stdout.trim_ascii_end()).unwrap()),
    ])
    .from(&fixture.linked.root)
    .succeeds();
}

/// Git can write its own diagnostic before git-dupe's refusal or warning.
fn after_git(output: &Output, git: &Output) -> Output {
    Output {
        stdout: output.stdout.clone(),
        stderr: output
            .stderr
            .strip_prefix(git.stderr.as_slice())
            .unwrap_or_else(|| panic!("Git's answer first: {git:?}; {output:?}"))
            .to_vec(),
        end: output.end,
    }
}

#[test]
fn linked_detach_refuses_each_unsafe_repository_without_changing_either_worktree() {
    under_each_release(|s| {
        for cause in ["modified", "added", "no-remote", "unreadable"] {
            let fixture = attached(s, cause);
            let wt = &fixture.linked;
            if cause != "no-remote" {
                pushed(s, &fixture, cause);
            }
            match cause {
                "modified" => write(&wt.root, "notes/x.md", b"changed\n"),
                "added" => {
                    write(&wt.root, "personal/new.md", b"new\n");
                    s.git(["dupe", "add", "personal/new.md"])
                        .from(&wt.root)
                        .succeeds();
                }
                "unreadable" => write(&wt.private_directory(), "HEAD", b"corrupt\n"),
                "no-remote" => {}
                _ => unreachable!(),
            }
            let status = wt
                .private(s)
                .git([
                    "--no-optional-locks",
                    "status",
                    "--porcelain",
                    "-z",
                    "--untracked-files=no",
                ])
                .run();
            if cause == "unreadable" {
                assert_ne!(status.end, End::Code(0), "{status:?}");
            } else {
                assert_eq!(status.end, End::Code(0), "{status:?}");
                assert_eq!(status.stdout.is_empty(), cause == "no-remote", "{status:?}");
            }
            let private = Tree::of(&wt.private_directory());
            let files = Tree::of(&wt.root);
            let public = wt.public_git();
            let exclude = fs::read(wt.common_directory.join("info/exclude")).unwrap();
            let output = s.git(["dupe", "detach"]).from(&wt.root).run();
            assert_eq!(output.end, End::Code(128), "{cause}: {output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let ours = after_git(&output, &status);
            let fatal = ours.only_line("fatal");
            names(fatal, b"git dupe detach --force");
            names(
                fatal,
                match cause {
                    "modified" | "added" => b"not committed".as_slice(),
                    "no-remote" => b"no remote is configured",
                    "unreadable" => b"cannot be read",
                    _ => unreachable!(),
                },
            );
            unchanged(&private, &wt.private_directory());
            unchanged(&files, &wt.root);
            assert!(public.changed_in(&wt.public_git()).is_empty());
            assert_eq!(
                fs::read(wt.common_directory.join("info/exclude")).unwrap(),
                exclude
            );
            assert!(wt.private_directory().is_dir());
            assert!(wt.region().is_some());
            fixture.sibling_unchanged();
        }
    });
}

#[test]
fn linked_detach_preserves_files_and_sibling_and_can_attach_again() {
    under_each_release(|s| {
        for force in [false, true] {
            let fixture = attached(s, if force { "forced" } else { "pushed" });
            let wt = &fixture.linked;
            if !force {
                pushed(s, &fixture, "clean");
            }
            let files = Tree::of(&wt.root);
            let public = wt.public_git();
            let words = if force {
                &["dupe", "detach", "--force"][..]
            } else {
                &["dupe", "detach"][..]
            };
            let output = s.git(words).from(&wt.root).succeeds();
            assert!(output.stdout.is_empty(), "{output:?}");
            let warnings = output.lines("warning");
            assert!(
                warnings.contains(&now_visible("personal").as_slice()),
                "{output:?}"
            );
            assert!(
                warnings.contains(&now_visible("personal/todo.md").as_slice()),
                "{output:?}"
            );
            assert!(
                !warnings
                    .iter()
                    .any(|line| holds(line, b"notes") && holds(line, b"visible to public Git")),
                "{output:?}"
            );
            fixture.detached();
            // Even the linked root's .git file is preserved: nothing is subtracted.
            unchanged(&files, &wt.root);
            assert!(wt.root.join(".gitdupe").is_file());
            assert!(public.changed_in(&wt.public_git()).is_empty());
            let visible = s
                .git(["status", "--porcelain", "--untracked-files=all"])
                .from(&wt.root)
                .succeeds();
            names(&visible.stdout, b"personal/todo.md");
            assert!(!holds(&visible.stdout, b"notes/x.md"), "{visible:?}");
            s.git(["dupe", "status"])
                .from(&fixture.main.root)
                .succeeds();
            fixture.sibling_unchanged();

            s.init(&wt.root);
            s.git(["dupe", "status"]).from(&wt.root).succeeds();
            s.git(["dupe", "status"])
                .from(&fixture.main.root)
                .succeeds();
            assert!(wt.private_directory().is_dir());
            assert_eq!(
                wt.region().unwrap().rules,
                [
                    b"/.gitdupe".to_vec(),
                    b"/notes".to_vec(),
                    b"/personal".to_vec()
                ]
            );
            assert_eq!(
                fixture.main.region().unwrap().rules,
                [b"/.gitdupe".to_vec(), b"/notes".to_vec()]
            );
            fixture.sibling_unchanged();
        }
    });
}

#[test]
fn linked_force_with_corrupt_index_uses_disk_listing_and_region() {
    under_each_release(|s| {
        let fixture = attached(s, "corrupt-index");
        let wt = &fixture.linked;
        write(&wt.private_directory(), "index", b"corrupt index\n");
        let listing = wt.private(s).git(["ls-files", "-z", "--full-name"]).run();
        assert_ne!(listing.end, End::Code(0), "{listing:?}");
        let files = Tree::of(&wt.root);
        let public = wt.public_git();
        let output = s
            .git(["dupe", "detach", "--force"])
            .from(&wt.root)
            .succeeds();
        let ours = after_git(&output, &listing);
        let warnings = ours.lines("warning");
        assert_eq!(
            warnings
                .iter()
                .filter(|line| holds(line, b"cannot list the private repository"))
                .count(),
            1,
            "{ours:?}"
        );
        assert!(
            warnings.contains(&now_visible("personal").as_slice()),
            "{ours:?}"
        );
        assert!(
            !warnings
                .iter()
                .any(|line| holds(line, b"notes") && holds(line, b"visible to public Git")),
            "{ours:?}"
        );
        fixture.detached();
        unchanged(&files, &wt.root);
        assert!(public.changed_in(&wt.public_git()).is_empty());
    });
}

#[test]
fn linked_detach_refuses_info_link_only_when_its_own_region_stands_beyond_it() {
    under_each_release(|s| {
        for own_region in [true, false] {
            let fixture = attached(
                s,
                if own_region {
                    "own-region"
                } else {
                    "main-only"
                },
            );
            let wt = &fixture.linked;
            let info = wt.common_directory.join("info");
            let target = s.dir().join(format!("info-target-{own_region}"));
            fs::rename(&info, &target).unwrap();
            if !own_region {
                let region = wt.region();
                assert!(region.is_none()); // The old info path is absent until linked back.
                let exclude = target.join("exclude");
                let bytes = fs::read(&exclude).unwrap();
                let own = fixture.linked.name().unwrap().as_encoded_bytes();
                let begin = [b"# BEGIN git-dupe worktree ".as_slice(), own, b"\n"].concat();
                let end = [b"# END git-dupe worktree ".as_slice(), own, b"\n"].concat();
                let start = bytes
                    .windows(begin.len())
                    .position(|part| part == begin)
                    .unwrap();
                let finish = bytes
                    .windows(end.len())
                    .position(|part| part == end)
                    .unwrap()
                    + end.len();
                fs::write(exclude, [&bytes[..start], &bytes[finish..]].concat()).unwrap();
            }
            symlink(&target, &info).unwrap();
            let beyond = Tree::of(&target);
            let private = Tree::of(&wt.private_directory());
            let files = Tree::of(&wt.root);
            if own_region {
                for words in [&["dupe", "detach"][..], &["dupe", "detach", "--force"]] {
                    let public = wt.public_git();
                    let output = s.git(words).from(&wt.root).run();
                    assert_eq!(output.end, End::Code(128), "{output:?}");
                    assert!(output.stdout.is_empty(), "{output:?}");
                    names(
                        output.only_line("fatal"),
                        info.as_os_str().as_encoded_bytes(),
                    );
                    unchanged(&private, &wt.private_directory());
                    assert!(wt.private_directory().is_dir());
                    assert!(wt.region().is_some());
                    assert!(public.changed_in(&wt.public_git()).is_empty());
                    fixture.sibling_unchanged();
                }
            } else {
                let public = wt.public_git();
                s.git(["dupe", "detach", "--force"])
                    .from(&wt.root)
                    .succeeds();
                fixture.detached();
                assert!(public.changed_in(&wt.public_git()).is_empty());
            }
            unchanged(&files, &wt.root);
            unchanged(&beyond, &target);
            assert_eq!(fs::read_link(&info).unwrap(), target);
            fixture.sibling_unchanged();
        }
    });
}

#[test]
fn linked_detach_failed_region_replacement_keeps_both_repositories_then_retries() {
    under_each_release(|s| {
        let fixture = attached(s, "failed-write");
        let wt = &fixture.linked;
        let private = Tree::of(&wt.private_directory());
        let files = Tree::of(&wt.root);
        let public = wt.public_git();
        let exclude = wt.common_directory.join("info/exclude");
        let bytes = fs::read(&exclude).unwrap();
        let output = s
            .killing("control", &["detach", "--force"])
            .writes_failing(&wt.root);
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let fatal = output.only_line("fatal");
        let prefix = [
            b"cannot delete the managed region of ".as_slice(),
            exclude.as_os_str().as_encoded_bytes(),
            b": ",
        ]
        .concat();
        assert!(fatal.starts_with(&prefix), "{output:?}");
        let cause = fatal
            .strip_prefix(prefix.as_slice())
            .unwrap()
            .strip_suffix(b"; nothing is detached")
            .unwrap();
        assert!(!cause.is_empty(), "{output:?}");
        assert_eq!(fs::read(&exclude).unwrap(), bytes);
        unchanged(&private, &wt.private_directory());
        unchanged(&files, &wt.root);
        assert!(wt.private_directory().is_dir());
        assert!(wt.region().is_some());
        assert!(public.changed_in(&wt.public_git()).is_empty());
        fixture.sibling_unchanged();
        let public = wt.public_git();
        s.git(["dupe", "detach", "--force"])
            .from(&wt.root)
            .succeeds();
        fixture.detached();
        unchanged(&files, &wt.root);
        assert!(public.changed_in(&wt.public_git()).is_empty());
    });
}

#[test]
fn linked_detach_completes_a_kill_after_region_deletion_and_partial_removal() {
    under_each_release(|s| {
        let killing = s.killing("control", &["detach", "--force"]);
        let fixture = attached(s, "uninterrupted");
        let public = fixture.linked.public_git();
        let (output, points) = killing.uninterrupted(&fixture.linked.root);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        fixture.detached();
        assert!(public.changed_in(&fixture.linked.public_git()).is_empty());
        let mut after_region = None;
        for (index, point) in points.into_iter().enumerate() {
            if !matches!(point, Point::BeforeRun(_) | Point::AfterRun(_)) {
                continue;
            }
            let fixture = attached(s, &format!("killed-{index}"));
            let wt = &fixture.linked;
            let files = Tree::of(&wt.root);
            let private = Tree::of(&wt.private_directory());
            let public = wt.public_git();
            killing.killed(&wt.root, point);
            assert!(wt.private_directory().is_dir());
            unchanged(&private, &wt.private_directory());
            fixture.sibling_unchanged();
            assert!(public.changed_in(&wt.public_git()).is_empty());
            let removed = wt.region().is_none();
            let public = wt.public_git();
            s.git(["dupe", "detach", "--force"])
                .from(&wt.root)
                .succeeds();
            fixture.detached();
            unchanged(&files, &wt.root);
            assert!(public.changed_in(&wt.public_git()).is_empty());
            if removed {
                after_region = Some(point);
                break;
            }
        }
        let after_region =
            after_region.expect("a kill after region deletion while the repository stands");
        // Recursive deletion has no Git run or write to kill inside. Reconstruct every
        // partial state used by detach_killed, on fresh linked fixtures instead of copies.
        for state in [
            "emptied",
            "HEAD",
            "objects",
            "refs",
            "objects-and-refs",
            "config-only",
            "index",
        ] {
            for listing_kept in [true, false] {
                let fixture = attached(s, &format!("partial-{state}-{listing_kept}"));
                let wt = &fixture.linked;
                let public = wt.public_git();
                killing.killed(&wt.root, after_region);
                assert_eq!(wt.region(), None);
                assert!(wt.private_directory().is_dir());
                fixture.sibling_unchanged();
                assert!(public.changed_in(&wt.public_git()).is_empty());
                let private = wt.private_directory();
                for entry in fs::read_dir(&private).unwrap() {
                    let entry = entry.unwrap();
                    let name = entry.file_name();
                    let remove = match state {
                        "emptied" => true,
                        "config-only" => name != "config",
                        "objects-and-refs" => name == "objects" || name == "refs",
                        _ => name == state,
                    };
                    if remove {
                        if entry.file_type().unwrap().is_dir() {
                            fs::remove_dir_all(entry.path()).unwrap();
                        } else {
                            fs::remove_file(entry.path()).unwrap();
                        }
                    }
                }
                if !listing_kept {
                    fs::remove_file(wt.root.join(".gitdupe")).unwrap();
                }
                assert!(private.is_dir());
                if state == "objects" {
                    assert!(private.join("HEAD").is_file());
                    assert!(!private.join("objects").exists());
                }
                let files = Tree::of(&wt.root);
                let public = wt.public_git();
                s.git(["dupe", "detach", "--force"])
                    .from(&wt.root)
                    .succeeds();
                fixture.detached();
                unchanged(&files, &wt.root);
                assert!(public.changed_in(&wt.public_git()).is_empty());
            }
        }
    });
}
