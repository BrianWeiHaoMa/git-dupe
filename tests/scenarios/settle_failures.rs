//! Settle's failures and its one snapshot of other worktrees' regions (G27,
//! Composition/Keeper). Real Git runs are forwarded except at the selected seam.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, ForwardEffect, Output, Point, Scenario, Tree, Worktree, holds, under_each_release, write,
};

const PUBLIC_LISTING: &[&str] = &["ls-files", "-z", "--full-name", "--"];
const PRIVATE_LISTING: &[&str] = &["ls-files", "-z", "--full-name"];
const QUESTION: &[&str] = &["check-ignore", "--no-index", "-v", "-n", "-z", "--stdin"];

struct Pair {
    main: Worktree,
    linked: Worktree,
    exclude: PathBuf,
}

fn pair(s: &Scenario, name: &str) -> Pair {
    let root = s.dir().join(name);
    s.attached_project(&root);
    let linked = s.dir().join(format!("{name}-linked"));
    s.linked_worktree(&root, &linked);
    s.init(&linked);
    // Keep the manifest present: the pause selects settle's private listing, without
    // a missing manifest's cat-file lookup before it.
    write(&root, ".gitdupe", b"");
    write(&linked, ".gitdupe", b"");
    let main = Worktree::read(s, &root);
    let linked = Worktree::read(s, &linked);
    let exclude = main.common_directory.join("info/exclude");
    Pair {
        main,
        linked,
        exclude,
    }
}

/// A pending own replacement and a stale peer, with all three warning kinds possible:
/// exposed is hidden but re-included, released stands, and foreign is ignored here.
struct Pending {
    pair: Pair,
    stale: Worktree,
    old: Vec<u8>,
    new: Vec<u8>,
}

fn pending(s: &Scenario, name: &str) -> Pending {
    let pair = pair(s, name);
    s.git(["dupe", "hide", "foreign"])
        .from(&pair.main.root)
        .succeeds();
    s.git(["dupe", "hide", "released"])
        .from(&pair.linked.root)
        .succeeds();
    let stale_root = s.dir().join(format!("{name}-stale"));
    s.linked_worktree(&pair.main.root, &stale_root);
    s.init(&stale_root);
    s.git(["dupe", "hide", "stale-path"])
        .from(&stale_root)
        .succeeds();
    let stale = Worktree::read(s, &stale_root);
    let old = fs::read(&pair.exclude).unwrap();
    let new = replaced_bytes(
        &replaced_bytes(&old, &stale.region_bytes(), b""),
        &pair.linked.region_bytes(),
        &region_bytes(&pair.linked, &[".gitdupe", "exposed"]),
    );
    fs::remove_dir_all(stale.private_directory()).unwrap();
    write(&pair.linked.root, ".gitdupe", b"exposed\n");
    write(&pair.linked.root, ".gitignore", b"!/exposed\n");
    for path in ["foreign", "exposed", "released"] {
        write(&pair.linked.root, path, b"here\n");
    }
    fs::set_permissions(&pair.exclude, fs::Permissions::from_mode(0o640)).unwrap();
    assert_ne!(old, new);
    assert_eq!(
        s.git(["check-ignore", "--no-index", "foreign"])
            .from(&pair.linked.root)
            .run()
            .end,
        End::Code(0)
    );
    assert_eq!(
        s.git(["check-ignore", "--no-index", "exposed"])
            .from(&pair.linked.root)
            .run()
            .end,
        End::Code(1)
    );
    Pending {
        pair,
        stale,
        old,
        new,
    }
}

fn replaced_bytes(bytes: &[u8], old: &[u8], new: &[u8]) -> Vec<u8> {
    let starts: Vec<_> = bytes
        .windows(old.len())
        .enumerate()
        .filter_map(|(at, part)| (part == old).then_some(at))
        .collect();
    assert_eq!(starts.len(), 1);
    let at = starts[0];
    [&bytes[..at], new, &bytes[at + old.len()..]].concat()
}

fn region_bytes(worktree: &Worktree, paths: &[&str]) -> Vec<u8> {
    let suffix = worktree.name().map_or(String::new(), |name| {
        format!(" worktree {}", name.to_str().unwrap())
    });
    let mut bytes = format!("# BEGIN git-dupe{suffix}\n").into_bytes();
    for path in paths {
        bytes.extend_from_slice(format!("/{path}\n").as_bytes());
    }
    bytes.extend_from_slice(format!("# END git-dupe{suffix}\n").as_bytes());
    bytes
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

fn failure_warning(output: &Output, before: &[u8], after: &[u8]) {
    let warnings = output.lines("warning");
    assert_eq!(warnings.len(), 1, "{output:?}");
    assert!(warnings[0].starts_with(before), "{output:?}");
    assert!(warnings[0].ends_with(after), "{output:?}");
}

/// Everything the failed settle must leave alone, including both indexes and files.
fn preserved(pending: &Pending) -> Tree {
    Tree::of(&pending.pair.main.common_directory).without(&[pending.pair.exclude.as_path()])
}

fn unchanged_repositories(before: &Tree, pending: &Pending) {
    let changed = before.changed_in(&preserved(pending));
    assert!(changed.is_empty(), "changed: {changed:?}");
}

/// An absent index is distinct from an empty index; none may be created by asking
/// about another worktree's paths or recovering from an interrupted replacement.
fn indexes(pair: &Pair) -> Vec<Option<Vec<u8>>> {
    [
        pair.main.git_directory.join("index"),
        pair.linked.git_directory.join("index"),
        pair.main.private_directory().join("index"),
        pair.linked.private_directory().join("index"),
    ]
    .map(|path| match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => None,
        Err(cause) => panic!("{}: {cause}", path.display()),
    })
    .into_iter()
    .collect()
}

fn still_stands(pending: &Pending) {
    assert!(pending.pair.main.private_directory().is_dir());
    assert!(pending.pair.linked.private_directory().is_dir());
    assert!(!pending.stale.private_directory().exists());
    for path in ["foreign", "exposed", "released"] {
        assert_eq!(
            fs::read(pending.pair.linked.root.join(path)).unwrap(),
            b"here\n"
        );
    }
    assert_eq!(
        fs::read(pending.pair.linked.root.join(".gitdupe")).unwrap(),
        b"exposed\n"
    );
    assert_eq!(mode(&pending.pair.exclude), 0o640);
}

#[test]
fn failed_public_listing_keeps_the_replacement_and_the_handlers_status() {
    under_each_release(|s| {
        for (name, words) in [
            ("successful-handler", &["status"][..]),
            ("failed-handler", &["log", "missing-revision"][..]),
        ] {
            let pending = pending(s, name);
            let pair = &pending.pair;
            let expected = pair.linked.private(s).git(words).run().end;
            assert_eq!(expected == End::Code(0), name == "successful-handler");
            let before = preserved(&pending);
            let main_region = pair.main.region_bytes();
            let forwarded = s
                .forwarded(
                    &format!("forward-{name}"),
                    words,
                    false,
                    PUBLIC_LISTING,
                    ForwardEffect::Exit(71),
                )
                .on_match(if words[0] == "status" { 2 } else { 1 });
            forwarded.observe(&pair.exclude, &pending.new);
            let output = forwarded.run(&pair.linked.root);
            forwarded.observed_equal();
            assert_eq!(output.end, expected, "{output:?}");
            failure_warning(
                &output,
                b"cannot list what public Git tracks under the hidden paths (git exited with 71)",
                b"; the managed region is replaced, but exposure was not checked",
            );
            assert_eq!(fs::read(&pair.exclude).unwrap(), pending.new);
            assert_eq!(pair.main.region_bytes(), main_region);
            assert!(pending.stale.region().is_none());
            unchanged_repositories(&before, &pending);
            still_stands(&pending);
            // Without the injected failure, all suppressed warning kinds really apply.
            write(&pair.linked.root, ".gitdupe", b"released\n");
            s.git(["dupe", "status"]).from(&pair.linked.root).succeeds();
            write(&pair.linked.root, ".gitdupe", b"exposed\n");
            let retry = s.git(["dupe", "status"]).from(&pair.linked.root).succeeds();
            assert_eq!(retry.lines("warning").len(), 3, "{retry:?}");
            for text in [
                b"exposed is hidden but public Git does not ignore it".as_slice(),
                b"released is no longer hidden and is visible to public Git",
                b"foreign stands here and is hidden by the main worktree alone",
            ] {
                assert!(
                    retry.lines("warning").iter().any(|line| holds(line, text)),
                    "{retry:?}"
                );
            }
        }
    });
}

#[test]
fn failed_or_truncated_ignore_answer_does_not_name_foreign_paths() {
    under_each_release(|s| {
        for (name, effect, cause) in [
            (
                "failed-question",
                ForwardEffect::Exit(72),
                "git exited with 72",
            ),
            (
                "truncated-question",
                ForwardEffect::Truncate,
                "git's answer cannot be read",
            ),
        ] {
            let pending = pending(s, name);
            let pair = &pending.pair;
            let before = preserved(&pending);
            let forwarded = s.forwarded(
                &format!("forward-{name}"),
                &["status"],
                false,
                QUESTION,
                effect,
            );
            let output = forwarded.run(&pair.linked.root);
            assert_eq!(output.end, End::Code(0), "{output:?}");
            failure_warning(
                &output,
                format!("cannot ask public Git whether it ignores the hidden paths ({cause})")
                    .as_bytes(),
                b"; exposure was not checked",
            );
            assert_eq!(fs::read(&pair.exclude).unwrap(), pending.new);
            unchanged_repositories(&before, &pending);
            still_stands(&pending);
        }
    });
}

#[test]
fn failed_region_write_keeps_the_whole_file_and_retry_prunes_stale_regions() {
    under_each_release(|s| {
        let pending = pending(s, "write-failure");
        let pair = &pending.pair;
        // G6 still asks exposure on a failed replacement. Keep every own and released
        // path ignored so that only the write warning remains; foreign stays standing.
        write(&pair.linked.root, ".gitignore", b"/exposed\n/released\n");
        let before = preserved(&pending);
        let killing = s.killing("failed-write", &["status"]);
        let output = killing.writes_failing(&pair.linked.root);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        failure_warning(
            &output,
            b"cannot replace the managed region of ",
            b"; private files may be visible to public Git",
        );
        assert_eq!(fs::read(&pair.exclude).unwrap(), pending.old);
        unchanged_repositories(&before, &pending);
        still_stands(&pending);
        let retry = s.git(["dupe", "status"]).from(&pair.linked.root).succeeds();
        assert_eq!(
            retry.lines("warning"),
            [foreign_warning("foreign").as_slice()],
            "{retry:?}"
        );
        assert_eq!(fs::read(&pair.exclude).unwrap(), pending.new);
        assert!(pending.stale.region().is_none());
        unchanged_repositories(&before, &pending);
        still_stands(&pending);
    });
}

#[test]
fn killed_region_write_leaves_a_whole_file_and_the_peer_can_proceed() {
    under_each_release(|s| {
        let pending = pending(s, "killed-write");
        let pair = &pending.pair;
        let main_region = pair.main.region_bytes();
        let before_indexes = indexes(pair);
        let killing = s.killing("inside-region-write", &["status"]);
        let killed = killing.start_killed(&pair.linked.root, Point::InsideWrite { blocks: 0 });
        killed.wait();
        let bytes = fs::read(&pair.exclude).unwrap();
        assert!(bytes == pending.old || bytes == pending.new);
        assert_eq!(indexes(pair), before_indexes);
        still_stands(&pending);
        // The killed run cannot keep the common-directory lock: this peer must end.
        let peer = s
            .git(["dupe", "status"])
            .from(&pair.main.root)
            .start()
            .wait_within(std::time::Duration::from_secs(30));
        assert_eq!(peer.end, End::Code(0), "{peer:?}");
        assert_eq!(pair.main.region_bytes(), main_region);
        assert!(pending.stale.region().is_none());
        let after_peer = if bytes == pending.old {
            replaced_bytes(
                &pending.old,
                &region_bytes(&pending.stale, &[".gitdupe", "stale-path"]),
                b"",
            )
        } else {
            pending.new.clone()
        };
        assert_eq!(fs::read(&pair.exclude).unwrap(), after_peer);
        assert_eq!(indexes(pair), before_indexes);
        s.git(["dupe", "status"]).from(&pair.linked.root).succeeds();
        assert_eq!(fs::read(&pair.exclude).unwrap(), pending.new);
        assert_eq!(indexes(pair), before_indexes);
        still_stands(&pending);
    });
}

fn foreign_warning(path: &str) -> Vec<u8> {
    format!("{path} stands here and is hidden by the main worktree alone: public Git ignores it here, and this worktree does not hide it; run from the root, 'git dupe hide -- {path}' hides it here too").into_bytes()
}

#[test]
fn foreign_snapshot_includes_a_peer_hide_before_the_replacement() {
    under_each_release(|s| {
        let pair = pair(s, "before-replacement");
        write(&pair.linked.root, "fresh", b"here\n");
        let forwarded = s
            .forwarded(
                "paused-private-listing",
                &["status"],
                true,
                PRIVATE_LISTING,
                ForwardEffect::Pause,
            )
            .on_match(2);
        let mut running = forwarded.start(&pair.linked.root);
        running.selected();
        let before = fs::read(&pair.exclude).unwrap();
        let peer = s
            .git(["dupe", "hide", "fresh"])
            .from(&pair.main.root)
            .start()
            .wait_within(std::time::Duration::from_secs(15));
        assert_eq!(peer.end, End::Code(0), "{peer:?}");
        let main_region = pair.main.region_bytes();
        let after_hide = fs::read(&pair.exclude).unwrap();
        assert_ne!(before, after_hide);
        let before_indexes = indexes(&pair);
        let output = running.release();
        assert_eq!(indexes(&pair), before_indexes);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(
            output.lines("warning"),
            [foreign_warning("fresh").as_slice()],
            "{output:?}"
        );
        assert_eq!(pair.main.region_bytes(), main_region);
        assert_eq!(fs::read(&pair.exclude).unwrap(), after_hide);
        assert_eq!(
            pair.linked.region_bytes(),
            region_bytes(&pair.linked, &[".gitdupe"])
        );
        assert_eq!(fs::read(pair.linked.root.join("fresh")).unwrap(), b"here\n");
        assert_eq!(
            pair.linked
                .private(s)
                .git(["ls-files", "-z"])
                .succeeds()
                .stdout,
            b""
        );
    });
}

#[test]
fn foreign_snapshot_survives_a_peer_release_and_hide_after_the_replacement() {
    under_each_release(|s| {
        let pair = pair(s, "after-replacement");
        s.git(["dupe", "hide", "old"])
            .from(&pair.main.root)
            .succeeds();
        for path in ["old", "new"] {
            write(&pair.linked.root, path, b"here\n");
        }
        // Both stay ignored after B changes its region: Git's live answer alone
        // cannot distinguish the snapshot's old path from the newly hidden one.
        write(&pair.linked.root, ".gitignore", b"/old\n/new\n");
        let before = fs::read(&pair.exclude).unwrap();
        let forwarded = s
            .forwarded(
                "paused-public-listing",
                &["status"],
                false,
                PUBLIC_LISTING,
                ForwardEffect::Pause,
            )
            .on_match(2);
        forwarded.observe(&pair.exclude, &before);
        let mut running = forwarded.start(&pair.linked.root);
        running.selected();
        // B's commands end before A resumes, proving the composition lock is released.
        for words in [["dupe", "unhide", "old"], ["dupe", "hide", "new"]] {
            let peer = s
                .git(words)
                .from(&pair.main.root)
                .start()
                .wait_within(std::time::Duration::from_secs(15));
            assert_eq!(peer.end, End::Code(0), "{peer:?}");
        }
        let main_region = pair.main.region_bytes();
        assert_eq!(main_region, region_bytes(&pair.main, &[".gitdupe", "new"]));
        let after_peer = fs::read(&pair.exclude).unwrap();
        let before_indexes = indexes(&pair);
        let output = running.release();
        assert_eq!(indexes(&pair), before_indexes);
        forwarded.observed_equal();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(
            output.lines("warning"),
            [foreign_warning("old").as_slice()],
            "{output:?}"
        );
        assert_eq!(pair.main.region_bytes(), main_region);
        assert_eq!(fs::read(&pair.exclude).unwrap(), after_peer);
        assert_eq!(
            pair.linked.region_bytes(),
            region_bytes(&pair.linked, &[".gitdupe"])
        );
        for path in ["old", "new"] {
            assert_eq!(fs::read(pair.linked.root.join(path)).unwrap(), b"here\n");
        }
        assert_eq!(
            pair.linked
                .private(s)
                .git(["ls-files", "-z"])
                .succeeds()
                .stdout,
            b""
        );
    });
}
