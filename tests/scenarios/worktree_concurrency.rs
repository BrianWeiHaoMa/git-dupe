//! The main worktree and a linked one of one project, each attached on its own, run their
//! commands at the same time (E2, G28, F3, G21, `Holds/G28`, R10): every update of each
//! worktree's region survives the other's, each worktree's hidden paths, `.gitdupe`, and
//! private history stay its own, and a command killed inside its write of
//! `.git/info/exclude` holds up none in the other worktree.
//!
//! Two commands started together are ordered by nothing more, so whether a pair's writes
//! of the file overlapped, and which took the lock first, is the machine's: what is
//! observed is the outcome after every pair. That both worktrees' commands wait for the
//! one lock on the common Git directory is observed with the harness holding it, as
//! `region_lock` observes it for the main worktree alone, and that none of them starts a
//! Git run while it holds the lock, from each Git run they start (R10).

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::harness::{
    End, Output, Point, Scenario, Tree, Worktree, commands_wait_for_the_lock, hold_the_lock,
    lock_is_free, now_visible, started_together, under_each_release, write,
};

/// The user's text before the main worktree's region, between the two regions, and after
/// them, without a newline at its end.
const TOP: &[u8] = b"# the user's own\n*.o\n";
const BETWEEN: &[u8] = b"# between the regions\n";
const LAST: &[u8] = b"# last, without a newline";

/// The file each worktree tracks privately, committed in its own private history.
const TRACKED: &str = "plans/today.md";

/// How many pairs of `hide` are started together.
const PAIRS: usize = 50;

/// How long a command is given to end once the lock is let go.
const LIMIT: Duration = Duration::from_secs(30);

/// The main worktree and a linked one of one project.
struct Project {
    main: Worktree,
    agent: Worktree,
}

impl Project {
    fn exclude(&self) -> PathBuf {
        self.main.common_directory.join("info/exclude")
    }
}

/// Makes the project `<name>-main` and its linked worktree `<name>-agent`, each attached
/// by `git dupe init` and tracking `TRACKED` in a private commit of its own, and their
/// exclude file the user's `top`, the main worktree's region, `BETWEEN`, the linked
/// worktree's region, and `LAST`.
fn project(s: &Scenario, name: &str, top: &[u8]) -> Project {
    let main = s.dir().join(format!("{name}-main"));
    let agent = s.dir().join(format!("{name}-agent"));
    s.repository(&main);
    s.linked_worktree(&main, &agent);
    for root in [&main, &agent] {
        s.init(root);
        write(root, TRACKED, b"private\n");
        s.git(["dupe", "add", TRACKED]).from(root).succeeds();
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
    let p = Project {
        main: Worktree::read(s, &main),
        agent: Worktree::read(s, &agent),
    };
    let regions = [p.main.region_bytes(), p.agent.region_bytes()];
    fs::write(p.exclude(), around(top, &regions[0], &regions[1])).unwrap();
    p
}

/// The exclude file of `project` holding the regions `main` and `agent`.
fn around(top: &[u8], main: &[u8], agent: &[u8]) -> Vec<u8> {
    [top, main, BETWEEN, agent, LAST].concat()
}

/// The rules of a region whose worktree lists `listed` in `.gitdupe` and tracks `TRACKED`:
/// one per path, `.gitdupe`'s among them, in byte order.
fn rules(listed: &[String]) -> Vec<Vec<u8>> {
    let paths: BTreeSet<&str> = listed
        .iter()
        .map(String::as_str)
        .chain([".gitdupe", TRACKED])
        .collect();
    paths
        .into_iter()
        .map(|path| format!("/{path}").into_bytes())
        .collect()
}

/// The region of `wt` holding `rules(listed)`, between that worktree's markers.
fn region_holding(wt: &Worktree, listed: &[String]) -> Vec<u8> {
    let mut begin = b"# BEGIN git-dupe".to_vec();
    let mut end = b"# END git-dupe".to_vec();
    if let Some(name) = wt.name() {
        for marker in [&mut begin, &mut end] {
            marker.extend_from_slice(b" worktree ");
            marker.extend_from_slice(name.as_bytes());
        }
    }
    let lines = [begin].into_iter().chain(rules(listed)).chain([end]);
    lines
        .flat_map(|line| [line, b"\n".to_vec()].concat())
        .collect()
}

/// `.gitdupe` of `wt` lists exactly `listed`, in that order, on disk and as staged in its
/// own private index.
fn lists(s: &Scenario, wt: &Worktree, listed: &[String]) {
    let listing: Vec<u8> = listed
        .iter()
        .flat_map(|path| [path.as_bytes(), b"\n"].concat())
        .collect();
    let on_disk = fs::read(wt.root.join(".gitdupe")).unwrap();
    assert!(
        on_disk == listing,
        "{wt:?}: .gitdupe holds \"{}\"",
        on_disk.escape_ascii()
    );
    let staged = wt
        .private(s)
        .git(["cat-file", "blob", ":.gitdupe"])
        .succeeds();
    assert!(
        staged.stdout == listing,
        "{wt:?}: .gitdupe is staged as \"{}\"",
        staged.stdout.escape_ascii()
    );
}

/// `wt` hides exactly `listed` beside `TRACKED`: `.gitdupe` as `lists` says, and its region
/// one rule for each hidden path, sorted, none twice and none of the other worktree's.
fn hides_exactly(s: &Scenario, wt: &Worktree, listed: &[String]) {
    lists(s, wt, listed);
    let region = wt.region().unwrap_or_else(|| panic!("no region of {wt:?}"));
    assert_eq!(region.rules, rules(listed), "{wt:?}");
}

/// The exclude file of `p` holds `expected`, byte for byte.
fn exclude_holds(p: &Project, expected: &[u8]) {
    let found = fs::read(p.exclude()).unwrap();
    assert!(
        found == expected,
        "the exclude file holds \"{}\", not \"{}\"",
        found.escape_ascii(),
        expected.escape_ascii()
    );
}

/// A command that ended with status 0 and no `warning:` or `fatal:` line.
fn ended_cleanly(output: &Output) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(
        output.lines("warning").is_empty() && output.lines("fatal").is_empty(),
        "{output:?}"
    );
}

/// What a command hiding a path in either worktree leaves as it was: the common Git
/// directory, the linked worktree's Git directory and each private history within it
/// included, but the two private Git directories and the exclude file; and each working
/// tree but its `.gitdupe`.
fn unrelated_to_hiding(p: &Project) -> Vec<Tree> {
    let common = Tree::of(&p.main.common_directory).without(&[
        &p.main.private_directory(),
        &p.agent.private_directory(),
        &p.exclude(),
    ]);
    let working =
        [&p.main, &p.agent].map(|wt| Tree::working(&wt.root).without(&[&wt.root.join(".gitdupe")]));
    [common].into_iter().chain(working).collect()
}

/// The refs of `wt`'s private repository and the commits they name.
fn private_refs(s: &Scenario, wt: &Worktree) -> Vec<u8> {
    let words = ["for-each-ref", "--format=%(refname) %(objectname)"];
    wt.private(s).git(words).succeeds().stdout
}

#[test]
fn both_worktrees_replace_and_delete_their_regions_under_the_one_lock() {
    under_each_release(|s| {
        let p = project(s, "lock", TOP);
        let common = &p.main.common_directory;
        let as_it_was = fs::read(p.exclude()).unwrap();
        let held = hold_the_lock(common);
        let mut main_hiding = s
            .git(["dupe", "hide", "main-notes"])
            .from(&p.main.root)
            .start();
        let mut agent_hiding = s
            .git(["dupe", "hide", "agent-notes"])
            .from(&p.agent.root)
            .start();
        commands_wait_for_the_lock(common, 2);
        assert!(!main_hiding.finished() && !agent_hiding.finished());
        exclude_holds(&p, &as_it_was);
        drop(held);
        ended_cleanly(&main_hiding.wait_within(LIMIT));
        ended_cleanly(&agent_hiding.wait_within(LIMIT));
        let (in_main, in_agent) = (["main-notes".to_owned()], ["agent-notes".to_owned()]);
        hides_exactly(s, &p.main, &in_main);
        hides_exactly(s, &p.agent, &in_agent);
        let main_region = region_holding(&p.main, &in_main);
        exclude_holds(
            &p,
            &around(TOP, &main_region, &region_holding(&p.agent, &in_agent)),
        );

        let held = hold_the_lock(common);
        let mut detaching = s
            .git(["dupe", "detach", "--force"])
            .from(&p.agent.root)
            .start();
        commands_wait_for_the_lock(common, 1);
        assert!(!detaching.finished());
        assert!(p.agent.region().is_some() && p.agent.private_directory().join("HEAD").is_file());
        drop(held);
        let detached = detaching.wait_within(LIMIT);
        assert_eq!(detached.end, End::Code(0), "{detached:?}");
        assert_eq!(p.agent.region(), None);
        assert!(fs::symlink_metadata(p.agent.private_directory()).is_err());
        exclude_holds(&p, &[TOP, &main_region[..], BETWEEN, LAST].concat());
    });
}

#[test]
fn no_worktree_starts_a_git_run_while_it_holds_the_lock() {
    under_each_release(|s| {
        let p = project(s, "watched", TOP);
        let third = s.dir().join("watched-third");
        s.linked_worktree(&p.main.root, &third);
        let watch = s.watching_the_lock("watched-control", &p.main.common_directory);
        ended_cleanly(&watch.run(&third, &["init"]));
        for (wt, path) in [(&p.main, "main-notes"), (&p.agent, "agent-notes")] {
            for words in [&["hide", path][..], &["unhide", path], &["status"]] {
                ended_cleanly(&watch.run(&wt.root, words));
            }
        }
        let detached = watch.run(&p.agent.root, &["detach", "--force"]);
        assert_eq!(detached.end, End::Code(0), "{detached:?}");
        assert_eq!(p.agent.region(), None);
    });
}

#[test]
fn fifty_pairs_of_hides_started_together_keep_every_path_in_its_own_region() {
    under_each_release(|s| {
        let p = project(s, "pairs", TOP);
        let unrelated = unrelated_to_hiding(&p);
        let refs = [&p.main, &p.agent].map(|wt| private_refs(s, wt));
        let (mut in_main, mut in_agent) = (Vec::new(), Vec::new());
        for n in 0..PAIRS {
            let (main_path, agent_path) = (format!("main-{n:02}"), format!("agent-{n:02}"));
            let (main_hid, agent_hid) = started_together(
                || {
                    s.git(["dupe", "hide", main_path.as_str()])
                        .from(&p.main.root)
                        .start()
                },
                || {
                    s.git(["dupe", "hide", agent_path.as_str()])
                        .from(&p.agent.root)
                        .start()
                },
            );
            ended_cleanly(&main_hid);
            ended_cleanly(&agent_hid);
            in_main.push(main_path);
            in_agent.push(agent_path);
            hides_exactly(s, &p.main, &in_main);
            hides_exactly(s, &p.agent, &in_agent);
        }
        let regions = [
            region_holding(&p.main, &in_main),
            region_holding(&p.agent, &in_agent),
        ];
        exclude_holds(&p, &around(TOP, &regions[0], &regions[1]));
        for (before, after) in unrelated.iter().zip(unrelated_to_hiding(&p)) {
            let changed = before.changed_in(&after);
            assert!(changed.is_empty(), "{changed:?}");
        }
        for (wt, refs) in [&p.main, &p.agent].into_iter().zip(refs) {
            assert_eq!(private_refs(s, wt), refs, "{wt:?}");
            let tracked = wt.private(s).git(["ls-files", "-z"]).succeeds();
            assert_eq!(tracked.stdout, b".gitdupe\0plans/today.md\0", "{wt:?}");
        }

        // The linked worktree detaches while the main one hides one more path: the
        // deletion of one region and the replacement of the other lose neither.
        write(&p.agent.root, "agent-00/kept.md", b"kept\n");
        let agent_files = Tree::working(&p.agent.root);
        let (main_hid, detached) = started_together(
            || {
                s.git(["dupe", "hide", "main-50"])
                    .from(&p.main.root)
                    .start()
            },
            || {
                s.git(["dupe", "detach", "--force"])
                    .from(&p.agent.root)
                    .start()
            },
        );
        ended_cleanly(&main_hid);
        assert_eq!(detached.end, End::Code(0), "{detached:?}");
        let warnings = detached.lines("warning");
        assert!(
            warnings.contains(&&now_visible("agent-00")[..]),
            "{detached:?}"
        );
        // The main worktree's region still hides `.gitdupe` there.
        assert!(
            !warnings.contains(&&now_visible(".gitdupe")[..]),
            "{detached:?}"
        );
        in_main.push("main-50".to_owned());
        hides_exactly(s, &p.main, &in_main);
        assert_eq!(p.agent.region(), None);
        assert!(fs::symlink_metadata(p.agent.private_directory()).is_err());
        let main_region = region_holding(&p.main, &in_main);
        exclude_holds(&p, &[TOP, &main_region[..], BETWEEN, LAST].concat());
        let changed = agent_files.changed_in(&Tree::working(&p.agent.root));
        assert!(changed.is_empty(), "{changed:?}");
    });
}

/// The user's comment above the regions, larger than a block of either size the kill
/// harness counts in, so that a write of the whole exclude file passes a limit of one
/// block before it ends.
fn comment() -> Vec<u8> {
    let comment: Vec<u8> = (0..100)
        .flat_map(|n| format!("# the user's comment, line {n}\n").into_bytes())
        .collect();
    assert!(comment.len() > 1024);
    comment
}

/// The names in the directory `directory`.
fn entries(directory: &Path) -> BTreeSet<OsString> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect()
}

/// In a fresh project `name`, a `hide` in one worktree, the main one when `main_killed`,
/// is killed inside its write of the exclude file, while a `hide` in the other, started
/// together with it, runs to its end; then the killed one is run again.
fn killed_beside_another_hide(s: &Scenario, name: &str, main_killed: bool) {
    let top = comment();
    let p = project(s, name, &top);
    let mut in_main = vec!["main-notes".to_owned()];
    let mut in_agent = vec!["agent-notes".to_owned()];
    for (wt, listed) in [(&p.main, &in_main), (&p.agent, &in_agent)] {
        let hid = s
            .git(["dupe", "hide", listed[0].as_str()])
            .from(&wt.root)
            .run();
        ended_cleanly(&hid);
    }
    hides_exactly(s, &p.main, &in_main);
    hides_exactly(s, &p.agent, &in_agent);
    let regions = |main: &[String], agent: &[String]| {
        around(
            &top,
            &region_holding(&p.main, main),
            &region_holding(&p.agent, agent),
        )
    };
    exclude_holds(&p, &regions(&in_main, &in_agent));

    let (target, peer) = match main_killed {
        true => (&p.main, &p.agent),
        false => (&p.agent, &p.main),
    };
    let (in_target, in_peer) = match main_killed {
        true => (&mut in_main, &mut in_agent),
        false => (&mut in_agent, &mut in_main),
    };
    let target_path = format!("{}-later", in_target[0]);
    let peer_path = format!("{}-later", in_peer[0]);
    in_target.push(target_path.clone());
    in_peer.push(peer_path.clone());
    // The killed command's `.gitdupe` is smaller than a block of either size, so that its
    // write of that file ends before the limit and the kill falls in the exclude file's.
    let listing_length: usize = in_target.iter().map(|path| path.len() + 1).sum();
    assert!(listing_length < 512);
    let private = target.private_directory();
    let private_before = entries(&private);

    let killing = s.killing(&format!("{name}-control"), &["hide", target_path.as_str()]);
    let (_killed, peer_hid) = started_together(
        || killing.start_killed(&target.root, Point::InsideWrite { blocks: 1 }),
        || {
            s.git(["dupe", "hide", peer_path.as_str()])
                .from(&peer.root)
                .start()
        },
    );
    ended_cleanly(&peer_hid);
    // The killed command wrote and staged `.gitdupe` whole, and was killed writing the
    // fresh exclude file in its own private Git directory, before the rename: its region
    // stands as it was, and the other worktree's holds that worktree's new path.
    lists(s, target, in_target);
    let left: Vec<OsString> = entries(&private)
        .difference(&private_before)
        .cloned()
        .collect();
    let [fresh] = &left[..] else {
        panic!("{left:?} were left in {}", private.display());
    };
    let fresh_name = fresh.as_bytes();
    assert!(
        fresh_name.starts_with(b"git-dupe-") && fresh_name.ends_with(b".new"),
        "{fresh:?}"
    );
    let partial = fs::read(private.join(fresh)).unwrap();
    assert!(
        partial.len() >= 512 && top.starts_with(&partial),
        "{fresh:?} is no exclude file cut short: \"{}\"",
        partial.escape_ascii()
    );
    hides_exactly(s, peer, in_peer);
    // The killed worktree's region is its old one, whole.
    let (main_then, agent_then) = match main_killed {
        true => (&in_main[..1], &in_agent[..]),
        false => (&in_main[..], &in_agent[..1]),
    };
    exclude_holds(&p, &regions(main_then, agent_then));
    assert!(
        lock_is_free(&p.main.common_directory),
        "the killed hide left the lock held"
    );

    // Run again, the killed command catches up, its path listed once, and the other
    // worktree's region stands as that worktree left it.
    let again = s
        .git(["dupe", "hide", target_path.as_str()])
        .from(&target.root)
        .run();
    ended_cleanly(&again);
    hides_exactly(s, &p.main, &in_main);
    hides_exactly(s, &p.agent, &in_agent);
    let both = regions(&in_main, &in_agent);
    exclude_holds(&p, &both);
    for wt in [&p.main, &p.agent] {
        let status = s.git(["dupe", "status"]).from(&wt.root).run();
        ended_cleanly(&status);
    }
    exclude_holds(&p, &both);
    // What the killed command left in its private Git directory is its own to leave.
    assert!(fs::read(private.join(fresh)).unwrap() == partial);
}

#[test]
fn a_hide_killed_inside_its_exclude_write_holds_up_none_in_the_other_worktree() {
    under_each_release(|s| {
        killed_beside_another_hide(s, "main-killed", true);
        killed_beside_another_hide(s, "agent-killed", false);
    });
}
