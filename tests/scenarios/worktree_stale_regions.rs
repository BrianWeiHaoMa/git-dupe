//! Stale regions disappear at the next composition, including an unchanged peer's
//! settle and a detach with no own region (G27). Interrupted attachments and detachments
//! never cause a peer to fabricate a region (G3, G27).

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Point, Scenario, Tree, Worktree, run_traced, unchanged, under_each_release, write,
};

const TOP: &[u8] = b"# user first\n*.o\n";
const BETWEEN: &[u8] = b"# user between\n";
const LAST: &[u8] = b"# user last without newline";

struct Regions {
    worktrees: Vec<Worktree>,
    regions: Vec<Vec<u8>>,
    exclude: PathBuf,
}

impl Regions {
    fn ordinary(s: &Scenario, name: &str) -> Self {
        let root = s.dir().join(name);
        s.attached_project(&root);
        let mut roots = vec![root.clone()];
        for name in ["gone", "peer"] {
            let linked = s.dir().join(format!(
                "{}-{name}",
                root.file_name().unwrap().to_str().unwrap()
            ));
            s.linked_worktree(&root, &linked);
            s.init(&linked);
            roots.push(linked);
        }
        Self::from_roots(s, &roots)
    }

    fn from_roots(s: &Scenario, roots: &[PathBuf]) -> Self {
        let worktrees: Vec<_> = roots.iter().map(|root| Worktree::read(s, root)).collect();
        for (n, wt) in worktrees.iter().enumerate() {
            write(&wt.root, ".gitdupe", b"");
            let note = format!("notes-{n}");
            write(&wt.root, &note, format!("private {n}\n").as_bytes());
            let add = s
                .git(["dupe", "add", ".gitdupe", &note])
                .from(&wt.root)
                .succeeds();
            assert!(add.lines("warning").is_empty(), "{add:?}");
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
                    "private notes",
                ])
                .succeeds();
        }
        let regions = worktrees.iter().map(Worktree::region_bytes).collect();
        let exclude = worktrees[0].common_directory.join("info/exclude");
        let fixture = Self {
            worktrees,
            regions,
            exclude,
        };
        fixture.install(&(0..roots.len()).collect::<Vec<_>>());
        fixture
    }

    fn bytes(&self, kept: &[usize]) -> Vec<u8> {
        let mut bytes = TOP.to_vec();
        for (n, region) in self.regions.iter().enumerate() {
            if kept.contains(&n) {
                bytes.extend(region);
            }
            // User text survives even when the preceding region disappears.
            bytes.extend(BETWEEN);
        }
        bytes.extend(LAST);
        bytes
    }

    fn install(&self, kept: &[usize]) {
        fs::write(&self.exclude, self.bytes(kept)).unwrap();
        fs::set_permissions(&self.exclude, fs::Permissions::from_mode(0o640)).unwrap();
        self.check(kept);
    }

    fn check(&self, kept: &[usize]) {
        assert_eq!(fs::read(&self.exclude).unwrap(), self.bytes(kept));
        assert_eq!(
            fs::metadata(&self.exclude).unwrap().permissions().mode() & 0o7777,
            0o640
        );
        for &n in kept {
            assert_eq!(self.worktrees[n].region_bytes(), self.regions[n]);
        }
    }

    fn survives(&self, s: &Scenario, kept: &[usize]) {
        for &n in kept {
            let wt = &self.worktrees[n];
            let listed = s.git(["dupe", "ls-files", "-z"]).from(&wt.root).succeeds();
            assert!(listed.lines("warning").is_empty(), "{listed:?}");
            assert_eq!(listed.stdout, format!(".gitdupe\0notes-{n}\0").as_bytes());
            let log = s
                .git(["dupe", "log", "-1", "--format=%s"])
                .from(&wt.root)
                .succeeds();
            assert_eq!(log.stdout, b"private notes\n");
            assert!(log.lines("warning").is_empty(), "{log:?}");
            let blob = wt
                .private(s)
                .git(["show", &format!("HEAD:notes-{n}")])
                .succeeds();
            assert_eq!(blob.stdout, format!("private {n}\n").as_bytes());
            assert_eq!(
                fs::read(wt.root.join(format!("notes-{n}"))).unwrap(),
                blob.stdout
            );
            let public = s.git(["ls-files", "-z"]).from(&wt.root).succeeds();
            assert_eq!(public.stdout, b".gitignore\0README.md\0docs/design.md\0");
        }
    }

    fn settle(&self, s: &Scenario, n: usize, kept: &[usize]) {
        let status = s
            .git(["dupe", "status"])
            .from(&self.worktrees[n].root)
            .succeeds();
        assert!(status.lines("warning").is_empty(), "{status:?}");
        self.check(kept);
    }
}

fn remove_worktree(s: &Scenario, repository: &Path, wt: &Worktree) {
    s.git([
        OsStr::new("worktree"),
        OsStr::new("remove"),
        wt.root.as_os_str(),
    ])
    .from(repository)
    .succeeds();
    assert!(!wt.root.exists());
    assert!(!wt.git_directory.exists());
}

#[test]
fn peer_settle_drops_regions_after_private_removal_worktree_removal_and_prune() {
    under_each_release(|s| {
        for removal in ["private", "worktree", "prune"] {
            let f = Regions::ordinary(s, removal);
            let gone = &f.worktrees[1];
            let main = &f.worktrees[0];
            let live = Tree::of(&main.private_directory());
            match removal {
                "private" => fs::remove_dir_all(gone.private_directory()).unwrap(),
                "worktree" => remove_worktree(s, &main.root, gone),
                "prune" => {
                    fs::remove_dir_all(&gone.root).unwrap();
                    assert!(gone.private_directory().is_dir());
                    s.git(["worktree", "prune", "--expire=now"])
                        .from(&main.root)
                        .succeeds();
                    assert!(!gone.git_directory.exists());
                }
                _ => unreachable!(),
            }
            // Removal itself does not edit exclude; the peer's own region is unchanged.
            f.check(&[0, 1, 2]);
            f.settle(s, 2, &[0, 2]);
            unchanged(&live, &main.private_directory());
            f.survives(s, &[0, 2]);
            f.check(&[0, 2]);
        }
    });
}

#[test]
fn private_files_and_links_are_stale_without_touching_the_link_target() {
    under_each_release(|s| {
        for kind in ["file", "dangling", "repository-link"] {
            let f = Regions::ordinary(s, kind);
            let private = f.worktrees[1].private_directory();
            fs::remove_dir_all(&private).unwrap();
            let target = s.dir().join(format!("target-{kind}"));
            s.bare_repository(&target);
            write(&target, "sentinel", b"untouched\n");
            let whole = Tree::of(&target);
            let destination = if kind == "dangling" {
                target.join("absent")
            } else {
                target.clone()
            };
            if kind == "file" {
                fs::write(&private, b"not a directory\n").unwrap();
            } else {
                symlink(&destination, &private).unwrap();
            }
            f.settle(s, 2, &[0, 2]);
            if kind == "file" {
                assert_eq!(fs::read(&private).unwrap(), b"not a directory\n");
            } else {
                assert!(fs::symlink_metadata(&private).unwrap().is_symlink());
                assert_eq!(fs::read_link(&private).unwrap(), destination);
            }
            unchanged(&whole, &target);
            f.survives(s, &[0, 2]);
            f.check(&[0, 2]);
            unchanged(&whole, &target);
        }
    });
}

#[test]
fn empty_and_unreadable_private_directories_keep_their_regions() {
    under_each_release(|s| {
        for junk in [false, true] {
            let f = Regions::ordinary(s, &format!("directory-{junk}"));
            let private = f.worktrees[1].private_directory();
            fs::remove_dir_all(&private).unwrap();
            fs::create_dir(&private).unwrap();
            if junk {
                write(&private, "HEAD", b"junk\n");
            }
            let unreadable = f.worktrees[1].private(s).git(["ls-files"]).run();
            assert_ne!(unreadable.end, End::Code(0), "{unreadable:?}");
            let whole = Tree::of(&private);
            f.settle(s, 2, &[0, 1, 2]);
            unchanged(&whole, &private);
            f.survives(s, &[0, 2]);
            f.check(&[0, 1, 2]);
        }
    });
}

#[test]
fn linked_settle_drops_the_removed_main_private_repositorys_region() {
    under_each_release(|s| {
        let f = Regions::ordinary(s, "main-gone");
        fs::remove_dir_all(f.worktrees[0].private_directory()).unwrap();
        f.check(&[0, 1, 2]);
        f.settle(s, 2, &[1, 2]);
        f.survives(s, &[1, 2]);
        f.check(&[1, 2]);
    });
}

#[test]
fn bare_worktree_removal_leaves_only_the_surviving_linked_region() {
    under_each_release(|s| {
        let project = s.dir().join("project");
        s.unattached_project(&project);
        let bare = s.dir().join("bare");
        s.git([
            OsStr::new("clone"),
            OsStr::new("--bare"),
            project.as_os_str(),
            bare.as_os_str(),
        ])
        .succeeds();
        let roots: Vec<_> = ["one", "two"].map(|name| s.dir().join(name)).into();
        for root in &roots {
            s.linked_worktree(&bare, root);
            s.init(root);
        }
        let f = Regions::from_roots(s, &roots);
        assert_eq!(f.exclude, bare.join("info/exclude"));
        remove_worktree(s, &bare, &f.worktrees[0]);
        f.check(&[0, 1]);
        f.settle(s, 1, &[1]);
        f.survives(s, &[1]);
        f.check(&[1]);
        assert!(!bare.join("dupe").exists());
    });
}

#[test]
fn peer_settle_never_fabricates_a_region_for_an_interrupted_linked_init() {
    under_each_release(|s| {
        let probe = s.dir().join("probe");
        s.unattached_project(&probe);
        let probe_link = s.dir().join("probe-link");
        s.linked_worktree(&probe, &probe_link);
        let (output, runs) = run_traced(
            s.git(["dupe", "init"]).from(&probe_link),
            &s.dir().join("init-trace"),
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let own = runs.own();
        let created = own
            .commands()
            .iter()
            .position(|word| *word == b"init")
            .unwrap()
            + 1;

        let f = Regions::ordinary(s, "interrupted-init");
        let root = s.dir().join("pending-init");
        s.linked_worktree(&f.worktrees[0].root, &root);
        let pending = Worktree::read(s, &root);
        s.killing("kill-init", &["init"])
            .killed(&root, Point::AfterRun(created));
        assert!(pending.private_directory().is_dir());
        assert!(pending.private_directory().join("HEAD").is_file());
        assert!(pending.region().is_none());
        f.check(&[0, 1, 2]);
        let whole = Tree::of(&pending.private_directory());
        f.settle(s, 2, &[0, 1, 2]);
        unchanged(&whole, &pending.private_directory());
        assert!(pending.region().is_none());
        f.survives(s, &[0, 1, 2]);
        f.check(&[0, 1, 2]);

        let completed = s.git(["dupe", "init"]).from(&root).succeeds();
        assert!(completed.lines("warning").is_empty(), "{completed:?}");
        assert!(
            completed
                .only_line("hint")
                .starts_with(b"this workspace was already attached; ")
        );
        assert_eq!(pending.region().unwrap().rules, [b"/.gitdupe".to_vec()]);
        let expected = [f.bytes(&[0, 1, 2]), b"\n".to_vec(), pending.region_bytes()].concat();
        assert_eq!(fs::read(&f.exclude).unwrap(), expected);
        assert_eq!(
            fs::metadata(&f.exclude).unwrap().permissions().mode() & 0o7777,
            0o640
        );
        s.git(["dupe", "init"]).from(&root).succeeds();
        assert_eq!(fs::read(&f.exclude).unwrap(), expected);
        f.survives(s, &[0, 1, 2]);
        assert_eq!(fs::read(&f.exclude).unwrap(), expected);
        assert_eq!(
            s.git(["dupe", "ls-files", "-z"])
                .from(&root)
                .succeeds()
                .stdout,
            b""
        );
    });
}

#[test]
fn interrupted_linked_detach_is_not_restored_and_finishes_without_an_own_region() {
    under_each_release(|s| {
        let probe = Regions::ordinary(s, "detach-probe");
        let (output, runs) = run_traced(
            s.git(["dupe", "detach", "--force"])
                .from(&probe.worktrees[1].root),
            &s.dir().join("detach-trace"),
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        // The public listing follows the region's deletion and precedes removal of the
        // private repository (Composition/Keeper Remove). The first listing is private.
        let own = runs.own();
        let listing = own
            .commands()
            .iter()
            .rposition(|word| *word == b"ls-files")
            .unwrap()
            + 1;
        assert_eq!(own.of("ls-files"), 2);
        for stale_main in [false, true] {
            let f = Regions::ordinary(s, &format!("interrupted-detach-{stale_main}"));
            let leaving = &f.worktrees[1];
            let whole = Tree::of(&leaving.private_directory());
            s.killing(&format!("kill-detach-{stale_main}"), &["detach", "--force"])
                .killed(&leaving.root, Point::BeforeRun(listing));
            assert!(leaving.region().is_none());
            assert!(leaving.private_directory().is_dir());
            unchanged(&whole, &leaving.private_directory());
            f.check(&[0, 2]);
            f.settle(s, 2, &[0, 2]);
            assert!(leaving.region().is_none());
            unchanged(&whole, &leaving.private_directory());
            if stale_main {
                fs::remove_dir_all(f.worktrees[0].private_directory()).unwrap();
            }
            // In the stale case this deletion has no own region, but must still compose
            // exclude and remove the main worktree's stale region (G27).
            let detached = s
                .git(["dupe", "detach", "--force"])
                .from(&leaving.root)
                .succeeds();
            assert!(!leaving.private_directory().exists());
            assert!(leaving.region().is_none());
            // G3: .gitdupe is still hidden by every live peer; only notes-1 is visible.
            let peer = format!(
                "worktree {}",
                f.worktrees[2].name().unwrap().to_str().unwrap()
            );
            let owners = if stale_main {
                peer
            } else {
                format!("the main worktree and {peer}")
            };
            let attribution = format!(
                ".gitdupe stands here and is hidden by {owners} alone: public Git ignores it here, and this worktree does not hide it"
            );
            assert_eq!(
                detached.lines("warning"),
                [
                    attribution.as_bytes(),
                    b"notes-1 was hidden and is now visible to public Git".as_slice(),
                ],
                "{detached:?}"
            );
            assert_eq!(detached.lines("hint"), [b"1 formerly hidden path named above is now visible to public Git; check 'git status' before 'git add -A' stages it".as_slice()]);
            let kept: &[usize] = if stale_main { &[2] } else { &[0, 2] };
            f.check(kept);
            f.survives(s, kept);
            f.check(kept);
            assert_eq!(
                fs::read(leaving.root.join("notes-1")).unwrap(),
                b"private 1\n"
            );
        }
    });
}
