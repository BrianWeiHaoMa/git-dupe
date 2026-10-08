//! Done when's bare-linked-worktree story: two Git-created roots clone one private
//! remote, preserve both exact regions during concurrent hides, and exchange a private
//! commit through push/pull. Shared exclusion also warns here (G2, G27, G28, F2, F3, S17).

use std::ffi::OsStr;
use std::fs;

use crate::harness::{
    End, Tree, Worktree, records, region_in, started_together, under_each_release, worktree_clone,
    worktree_dupe, worktree_private_head, worktree_public_clean, write,
};

fn regions(one: &Worktree, two: &Worktree, paths: &[Vec<String>; 2]) {
    for (wt, paths) in [one, two].into_iter().zip(paths) {
        let mut rules = vec![
            b"/.env.local".to_vec(),
            b"/.gitdupe".to_vec(),
            b"/.vscode".to_vec(),
            b"/docs/notes.md".to_vec(),
            b"/notes".to_vec(),
        ];
        rules.extend(paths.iter().map(|path| format!("/{path}").into_bytes()));
        rules.sort();
        assert_eq!(wt.region().unwrap().rules, rules);
        let name = wt.name().unwrap().to_str().unwrap();
        let mut bytes = format!("# BEGIN git-dupe worktree {name}\n").into_bytes();
        for rule in rules {
            bytes.extend(rule);
            bytes.push(b'\n');
        }
        bytes.extend(format!("# END git-dupe worktree {name}\n").as_bytes());
        assert_eq!(wt.region_bytes(), bytes);
    }
    assert!(region_in(&one.common_directory, None).is_none());
    let exclude = fs::read(one.common_directory.join("info/exclude")).unwrap();
    assert_eq!(
        records(&exclude, b'\n')
            .iter()
            .filter(|line| line.starts_with(b"# BEGIN git-dupe"))
            .count(),
        2
    );
    assert_eq!(
        records(&exclude, b'\n')
            .iter()
            .filter(|line| line.starts_with(b"# END git-dupe"))
            .count(),
        2
    );
}

#[test]
fn bare_linked_worktrees_keep_concurrent_regions_and_exchange_private_history() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let remote = s.pushed_workspace("project");
        s.git(["symbolic-ref", "HEAD", "refs/heads/main"])
            .from(&remote.private_remote)
            .succeeds();
        let bare = s.dir().join("project.git");
        s.git([
            OsStr::new("clone"),
            OsStr::new("--bare"),
            remote.root.as_os_str(),
            bare.as_os_str(),
        ])
        .succeeds();
        let worktrees = ["agent", "agent-2"].map(|name| {
            let root = s.dir().join(name);
            s.linked_worktree(&bare, &root);
            let wt = Worktree::read(s, &root);
            assert_eq!(wt.name(), Some(OsStr::new(name)));
            assert_eq!(wt.common_directory, bare);
            assert_eq!(
                wt.private_directory(),
                bare.join("worktrees").join(name).join("dupe")
            );
            worktree_clone(s, &wt, &remote.private_remote);
            assert!(wt.private_directory().join("HEAD").is_file());
            assert_eq!(
                worktree_private_head(s, &wt),
                s.private(&remote.root)
                    .git(["rev-parse", "HEAD"])
                    .succeeds()
                    .stdout
            );
            assert_eq!(
                wt.private(s)
                    .git(["ls-files", "-s", "-z"])
                    .succeeds()
                    .stdout,
                s.private(&remote.root)
                    .git(["ls-files", "-s", "-z"])
                    .succeeds()
                    .stdout
            );
            for path in [
                ".gitdupe",
                ".env.local",
                "notes/a.md",
                ".vscode/settings.json",
                "docs/notes.md",
            ] {
                assert_eq!(
                    fs::read(root.join(path)).unwrap(),
                    fs::read(remote.root.join(path)).unwrap()
                );
            }
            worktree_public_clean(s, &wt);
            wt
        });
        let [one, two] = &worktrees;
        let mut paths: [Vec<String>; 2] = [Vec::new(), Vec::new()];
        regions(one, two, &paths);
        let public = || {
            Tree::of(&bare).without(&[
                &one.private_directory(),
                &two.private_directory(),
                &bare.join("info/exclude"),
            ])
        };
        let before = public();
        for pair in 0..6 {
            let names = [format!("one-{pair}"), format!("two-{pair}")];
            let (first, second) = started_together(
                || {
                    s.git(["dupe", "hide", names[0].as_str()])
                        .from(&one.root)
                        .start()
                },
                || {
                    s.git(["dupe", "hide", names[1].as_str()])
                        .from(&two.root)
                        .start()
                },
            );
            for output in [first, second] {
                assert_eq!(output.end, End::Code(0), "{output:?}");
                assert!(output.lines("warning").is_empty(), "{output:?}");
            }
            for (paths, name) in paths.iter_mut().zip(names) {
                paths.push(name);
            }
            regions(one, two, &paths);
            assert!(before.changed_in(&public()).is_empty());
        }

        // The concurrent declarations are each staged in .gitdupe. Release these
        // unused paths before sharing content, returning both indexes to the remote's
        // declaration so pull meets a clean peer rather than a local staged edit.
        for (wt, paths) in worktrees.iter().zip(&paths) {
            for path in paths {
                worktree_dupe(s, wt, &["unhide", path]);
            }
            wt.private(s)
                .git(["diff", "--cached", "--exit-code"])
                .succeeds();
            wt.private(s).git(["diff", "--exit-code"]).succeeds();
        }
        regions(one, two, &[Vec::new(), Vec::new()]);
        write(&one.root, "notes/a.md", b"agent's private contribution\n");
        worktree_dupe(s, one, &["add", "notes/a.md"]);
        worktree_dupe(s, one, &["commit", "-m", "Private contribution"]);
        let commit = worktree_private_head(s, one);
        let peer_history = Tree::of(&two.private_directory());
        let peer_files = Tree::working(&two.root);
        let peer_region = two.region_bytes();
        worktree_dupe(s, one, &["push"]);
        assert!(
            peer_history
                .changed_in(&Tree::of(&two.private_directory()))
                .is_empty()
        );
        assert!(peer_files.changed_in(&Tree::working(&two.root)).is_empty());
        assert_eq!(two.region_bytes(), peer_region);
        worktree_dupe(s, two, &["pull"]);
        assert_eq!(worktree_private_head(s, two), commit);
        assert_eq!(worktree_private_head(s, one), commit);
        for wt in &worktrees {
            assert_eq!(
                fs::read(wt.root.join("notes/a.md")).unwrap(),
                b"agent's private contribution\n"
            );
            assert_eq!(
                wt.private(s)
                    .git(["cat-file", "blob", ":notes/a.md"])
                    .succeeds()
                    .stdout,
                b"agent's private contribution\n"
            );
            worktree_public_clean(s, wt);
        }
        assert_eq!(
            s.git(["rev-parse", "refs/heads/main"])
                .from(&remote.private_remote)
                .succeeds()
                .stdout,
            commit
        );
        regions(one, two, &[Vec::new(), Vec::new()]);
        assert!(before.changed_in(&public()).is_empty());

        worktree_dupe(s, one, &["hide", "scratch/"]);
        write(&two.root, "scratch/x.md", b"invisible to both indexes\n");
        worktree_public_clean(s, two);
        let peer_index = two
            .private(s)
            .git(["ls-files", "-s", "-z"])
            .succeeds()
            .stdout;
        let peer_files = Tree::working(&two.root);
        let warned = worktree_dupe(s, two, &["status", "--porcelain"]);
        assert_eq!(warned.lines("warning"), [b"scratch stands here and is hidden by worktree agent alone: public Git ignores it here, and this worktree does not hide it; run from the root, 'git dupe hide -- scratch' hides it here too".as_slice()], "{warned:?}");
        assert!(warned.stdout.is_empty(), "{warned:?}");
        assert_eq!(
            two.private(s)
                .git(["ls-files", "-s", "-z"])
                .succeeds()
                .stdout,
            peer_index
        );
        assert!(peer_files.changed_in(&Tree::working(&two.root)).is_empty());
        regions(one, two, &[vec!["scratch".to_owned()], Vec::new()]);
        assert!(before.changed_in(&public()).is_empty());
    });
}
