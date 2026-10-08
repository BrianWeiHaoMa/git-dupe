//! Detach attributes formerly hidden paths to the live regions that still hide them
//! (G3, G27), including a retry after the region has already been deleted.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use crate::harness::{
    End, ForwardEffect, Output, Point, Scenario, Tree, Worktree, holds, names, names_number,
    now_visible, records, unchanged, under_each_release, warnings, write,
};

struct Attached {
    main: Worktree,
    linked: Worktree,
    main_private: Tree,
    main_region: Vec<u8>,
    exclude_without_linked: Vec<u8>,
}

fn attached(s: &Scenario, name: &str) -> Attached {
    let main = s.dir().join(format!("{name}-main"));
    s.attached_project(&main);
    s.git(["rm", "--cached", "--", "docs/design.md"])
        .from(&main)
        .succeeds();
    s.commit_public(&main);
    fs::create_dir(main.join("notes")).unwrap();
    s.git(["dupe", "hide", "notes", "docs"])
        .from(&main)
        .succeeds();
    let linked = s.dir().join(format!("{name}-linked"));
    s.linked_worktree(&main, &linked);
    s.init(&linked);
    fs::create_dir(linked.join("notes")).unwrap();
    for path in ["docs/a.md", "notes-old", "tracked", "build.log"] {
        write(&linked, path, b"kept\n");
    }
    write(&linked, ".gitignore", b"/build.log\n");
    s.git(["dupe", "hide", "notes", "notes-old", "tracked", "build.log"])
        .from(&linked)
        .succeeds();
    s.git([
        "dupe",
        "add",
        "-f",
        "docs/a.md",
        "notes-old",
        "tracked",
        "build.log",
    ])
    .from(&linked)
    .succeeds();
    s.git(["add", "-f", "--", "tracked"])
        .from(&linked)
        .succeeds();
    let main = Worktree::read(s, &main);
    let linked = Worktree::read(s, &linked);
    let private = linked.private(s).git(["ls-files", "-z"]).succeeds();
    assert_eq!(
        records(&private.stdout, 0),
        [
            b".gitdupe".as_slice(),
            b"build.log",
            b"docs/a.md",
            b"notes-old",
            b"tracked"
        ]
    );
    // The fallback must get a.md from the region, not the disk listing (Holds/G3).
    assert!(!holds(
        &fs::read(linked.root.join(".gitdupe")).unwrap(),
        b"docs/a.md"
    ));
    assert!(
        linked
            .region()
            .unwrap()
            .rules
            .contains(&b"/docs/a.md".to_vec())
    );
    assert_eq!(
        main.region().unwrap().rules,
        [b"/.gitdupe".to_vec(), b"/docs".to_vec(), b"/notes".to_vec()]
    );
    let own = linked.region().unwrap();
    let exclude_without_linked = [own.before, own.after].concat();
    Attached {
        exclude_without_linked,
        main_private: Tree::of(&main.private_directory()),
        main_region: main.region_bytes(),
        main,
        linked,
    }
}

fn attributed(path: &str) -> Vec<u8> {
    format!("{path} stands here and is hidden by the main worktree alone: public Git ignores it here, and this worktree does not hide it").into_bytes()
}

fn attribution(output: &Output) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    warnings(
        output,
        vec![
            attributed(".gitdupe"),
            attributed("notes"),
            attributed("docs/a.md"),
            now_visible("notes-old"),
            now_visible("tracked"),
        ],
    );
}

impl Attached {
    fn sibling_unchanged(&self) {
        unchanged(&self.main_private, &self.main.private_directory());
        assert_eq!(self.main.region_bytes(), self.main_region);
        assert!(self.main.private_directory().is_dir());
    }

    fn detached(&self) {
        assert_eq!(self.linked.region(), None);
        assert_eq!(
            fs::read(self.linked.common_directory.join("info/exclude")).unwrap(),
            self.exclude_without_linked
        );
        assert!(!self.linked.private_directory().exists());
        self.sibling_unchanged();
    }
}

#[test]
fn foreign_detach_attributes_equal_and_descendant_paths_but_counts_only_visible_paths() {
    under_each_release(|s| {
        let fixture = attached(s, "attribution");
        let wt = &fixture.linked;
        let files = Tree::of(&wt.root);
        let public = wt.public_git();
        let output = s
            .git(["dupe", "detach", "--force"])
            .from(&wt.root)
            .succeeds();
        attribution(&output);
        fixture.detached();
        unchanged(&files, &wt.root);
        assert!(public.changed_in(&wt.public_git()).is_empty());
        let ignored = s
            .git([
                "check-ignore",
                "--",
                ".gitdupe",
                "notes",
                "docs/a.md",
                "build.log",
            ])
            .from(&wt.root)
            .succeeds();
        assert_eq!(
            records(&ignored.stdout, b'\n'),
            [b".gitdupe".as_slice(), b"notes", b"docs/a.md", b"build.log"]
        );
        assert_eq!(
            s.git(["ls-files", "--", "tracked"])
                .from(&wt.root)
                .succeeds()
                .stdout,
            b"tracked\n"
        );
    });
}

#[test]
fn foreign_detach_with_corrupt_index_attributes_disk_and_region_paths() {
    under_each_release(|s| {
        let fixture = attached(s, "corrupt");
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
        let own = output
            .stderr
            .strip_prefix(listing.stderr.as_slice())
            .unwrap();
        let first_end = own.iter().position(|&b| b == b'\n').unwrap() + 1;
        let first = &own[..first_end];
        assert!(first.starts_with(b"warning: cannot list the private repository ("));
        assert!(first.ends_with(b"); the paths asked about are .gitdupe, the paths it lists, and those of the managed region\n"));
        attribution(&Output {
            stdout: output.stdout,
            stderr: own[first_end..].to_vec(),
            end: output.end,
        });
        fixture.detached();
        unchanged(&files, &wt.root);
        assert!(public.changed_in(&wt.public_git()).is_empty());
    });
}

#[test]
fn foreign_detach_with_a_failed_public_listing_names_no_path_as_hidden_elsewhere() {
    under_each_release(|s| {
        // `docs/a.md` is tracked by both repositories here and lies below the main
        // worktree's `docs`: public Git sees it, though the main region's rule matches it.
        let track_publicly = |fixture: &Attached| {
            s.git(["add", "-f", "--", "docs/a.md"])
                .from(&fixture.linked.root)
                .succeeds();
        };
        let listed = attached(s, "listed");
        track_publicly(&listed);
        let output = s
            .git(["dupe", "detach", "--force"])
            .from(&listed.linked.root)
            .succeeds();
        warnings(
            &output,
            vec![
                attributed(".gitdupe"),
                attributed("notes"),
                now_visible("docs/a.md"),
                now_visible("notes-old"),
                now_visible("tracked"),
            ],
        );
        listed.detached();

        // Without the listing, whether public Git tracks a path is not known, so no path is
        // named as one it ignores because another worktree hides it (G3, G7, G25).
        let unlisted = attached(s, "unlisted");
        track_publicly(&unlisted);
        let wt = &unlisted.linked;
        let files = Tree::of(&wt.root);
        let forwarded = s.forwarded(
            "listing-control",
            &["detach", "--force"],
            false,
            &["ls-files", "-z", "--full-name", "--"],
            ForwardEffect::Exit(71),
        );
        let output = forwarded.run(&wt.root);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        warnings(
            &output,
            vec![
                b"cannot list what public Git tracks under the hidden paths (git exited with \
                  71); a path it tracks is visible to it and may not be named, and none is \
                  named as hidden by another worktree"
                    .to_vec(),
                now_visible("notes-old"),
                now_visible("tracked"),
            ],
        );
        unlisted.detached();
        unchanged(&files, &wt.root);
    });
}

#[test]
fn foreign_detach_oversized_public_query_keeps_repository_after_region_deletion_then_retries() {
    under_each_release(|s| {
        let fixture = attached(s, "oversized");
        let wt = &fixture.linked;
        let limit = Command::new("getconf").arg("ARG_MAX").output().unwrap();
        assert!(limit.status.success());
        let limit: usize = std::str::from_utf8(&limit.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let count = limit / 128 + 1;
        let mut listing = fs::read(wt.root.join(".gitdupe")).unwrap();
        let mut extra = Vec::new();
        for index in 0..count {
            let path = format!("path-{index:016}-{}", "x".repeat(128));
            write(&wt.root, &path, b"kept\n");
            listing.extend_from_slice(path.as_bytes());
            listing.push(b'\n');
            extra.push(path);
        }
        fs::write(wt.root.join(".gitdupe"), listing).unwrap();
        let files = Tree::of(&wt.root);
        let private = Tree::of(&wt.private_directory());
        let public = wt.public_git();
        let output = s.git(["dupe", "detach", "--force"]).from(&wt.root).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let fatal = output.lines("fatal");
        assert_eq!(fatal.len(), 1);
        names_number(fatal[0], count + 6);
        assert_eq!(fatal[0], format!("a list of {} paths does not fit on one command line for Git, and git-dupe never shortens one", count + 6).as_bytes());
        let mut expected = vec![
            attributed(".gitdupe"),
            attributed("notes"),
            attributed("docs/a.md"),
            now_visible("notes-old"),
            now_visible("tracked"),
        ];
        expected.extend(extra.iter().map(|path| now_visible(path)));
        // The listing cannot spawn, but detach still asks check-ignore through stdin.
        // A failed detach has no closing hint: its private repository still stands.
        let mut found: Vec<Vec<u8>> = output
            .lines("warning")
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect();
        found.sort();
        expected.sort();
        assert_eq!(found, expected);
        assert!(output.lines("hint").is_empty());
        assert_eq!(
            output.stderr.split_inclusive(|&b| b == b'\n').count(),
            count + 6
        );
        assert_eq!(wt.region(), None);
        assert_eq!(
            fs::read(wt.common_directory.join("info/exclude")).unwrap(),
            fixture.exclude_without_linked
        );
        assert!(wt.private_directory().is_dir());
        unchanged(&private, &wt.private_directory());
        unchanged(&files, &wt.root);
        assert!(public.changed_in(&wt.public_git()).is_empty());
        fixture.sibling_unchanged();
        // Remove only the query-size fixture: all formerly hidden domain paths stand.
        for path in extra {
            fs::remove_file(wt.root.join(path)).unwrap();
        }
        let files = Tree::of(&wt.root);
        attribution(
            &s.git(["dupe", "detach", "--force"])
                .from(&wt.root)
                .succeeds(),
        );
        fixture.detached();
        unchanged(&files, &wt.root);
        assert!(public.changed_in(&wt.public_git()).is_empty());
    });
}

#[test]
fn foreign_detach_killed_after_region_deletion_still_attributes_on_retry() {
    under_each_release(|s| {
        let killing = s.killing("kill-control", &["detach", "--force"]);
        let baseline = attached(s, "baseline");
        let (output, points) = killing.uninterrupted(&baseline.linked.root);
        attribution(&output);
        baseline.detached();
        let mut found = false;
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
            unchanged(&files, &wt.root);
            fixture.sibling_unchanged();
            assert!(public.changed_in(&wt.public_git()).is_empty());
            let removed = wt.region().is_none();
            attribution(
                &s.git(["dupe", "detach", "--force"])
                    .from(&wt.root)
                    .succeeds(),
            );
            fixture.detached();
            unchanged(&files, &wt.root);
            assert!(public.changed_in(&wt.public_git()).is_empty());
            if removed {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "a kill between region deletion and repository removal"
        );
    });
}

#[test]
fn foreign_detach_failed_write_preserves_stale_and_live_regions_then_retry_drops_stale() {
    under_each_release(|s| {
        let fixture = attached(s, "write-failure");
        let wt = &fixture.linked;
        wt.private(s)
            .git([
                "-c",
                "maintenance.auto=false",
                "-c",
                "user.name=Scenario",
                "-c",
                "user.email=scenario@example.invalid",
                "commit",
                "-qm",
                "private files",
            ])
            .succeeds();
        let remote = s.dir().join("remote.git");
        s.bare_repository(&remote);
        wt.private(s)
            .git([
                OsStr::new("remote"),
                OsStr::new("add"),
                OsStr::new("origin"),
                remote.as_os_str(),
            ])
            .succeeds();
        wt.private(s)
            .git(["push", "-u", "origin", "HEAD"])
            .succeeds();
        let stale_root = s.dir().join("stale");
        s.linked_worktree(&fixture.main.root, &stale_root);
        s.init(&stale_root);
        write(&stale_root, "stale-only", b"stale\n");
        s.git(["dupe", "hide", "stale-only"])
            .from(&stale_root)
            .succeeds();
        let stale = Worktree::read(s, &stale_root);
        fs::remove_dir_all(stale.private_directory()).unwrap();
        let stale_region = stale.region_bytes();
        let exclude = wt.common_directory.join("info/exclude");
        fs::set_permissions(&exclude, fs::Permissions::from_mode(0o640)).unwrap();
        let bytes = fs::read(&exclude).unwrap();
        let mode = fs::metadata(&exclude).unwrap().permissions().mode();
        let private = Tree::of(&wt.private_directory());
        let files = Tree::of(&wt.root);
        let public = wt.public_git();
        let output = s
            .killing("write-control", &["detach"])
            .writes_failing(&wt.root);
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let fatal = output.only_line("fatal");
        names(fatal, b"cannot delete the managed region of ");
        names(fatal, exclude.as_os_str().as_encoded_bytes());
        assert!(fatal.ends_with(b"; nothing is detached"), "{output:?}");
        assert_eq!(fs::read(&exclude).unwrap(), bytes);
        assert_eq!(fs::metadata(&exclude).unwrap().permissions().mode(), mode);
        assert_eq!(stale.region_bytes(), stale_region);
        assert!(wt.region().is_some());
        assert!(wt.private_directory().is_dir());
        unchanged(&private, &wt.private_directory());
        unchanged(&files, &wt.root);
        fixture.sibling_unchanged();
        assert!(public.changed_in(&wt.public_git()).is_empty());
        attribution(&s.git(["dupe", "detach"]).from(&wt.root).succeeds());
        fixture.detached();
        assert_eq!(stale.region(), None);
        assert!(!stale.private_directory().exists());
        assert_eq!(fs::metadata(&exclude).unwrap().permissions().mode(), mode);
        unchanged(&files, &wt.root);
        assert!(public.changed_in(&wt.public_git()).is_empty());
    });
}
