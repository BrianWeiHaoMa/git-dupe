use super::*;

fn read(answer: &[u8]) -> Workspace {
    Workspace::read(answer, false).expect("four answers")
}

#[test]
fn at_the_root_the_prefix_line_is_empty() {
    let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\n\n");
    assert_eq!(workspace.git_directory, Path::new("/w/repo/.git"));
    assert_eq!(workspace.common_directory(), Path::new("/w/repo/.git"));
    assert_eq!(workspace.root(), Path::new("/w/repo"));
    assert_eq!(
        workspace.private_directory(),
        Path::new("/w/repo/.git/dupe")
    );
    assert_eq!(workspace.prefix(), b"");
    assert!(!workspace.linked());
    assert!(!workspace.attached());
}

#[test]
fn below_the_root_the_answer_names_the_same_repository() {
    let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/dir/\n");
    assert_eq!(workspace.git_directory, Path::new("/w/repo/.git"));
    assert_eq!(workspace.root(), Path::new("/w/repo"));
    assert_eq!(workspace.prefix(), b"sub/dir/");
}

#[test]
fn a_linked_worktrees_private_repository_is_in_its_own_git_directory() {
    // An ordinary repository's, a bare one's, and one made with `--separate-git-dir`:
    // never the main worktree's, whatever the common Git directory is called.
    for (answer, private) in [
        (
            &b"/w/repo/.git/worktrees/linked\n/w/repo/.git\n/w/linked\nsub/\n"[..],
            "/w/repo/.git/worktrees/linked/dupe",
        ),
        (
            b"/w/bare.git/worktrees/one\n/w/bare.git\n/w/one\n\n",
            "/w/bare.git/worktrees/one/dupe",
        ),
        (
            b"/w/s/admin/worktrees/linked\n/w/s/admin\n/w/s/linked\n\n",
            "/w/s/admin/worktrees/linked/dupe",
        ),
    ] {
        let workspace = read(answer);
        assert!(workspace.linked(), "{}", answer.escape_ascii());
        assert_eq!(
            workspace.private_directory(),
            Path::new(private),
            "{}",
            answer.escape_ascii()
        );
    }
}

#[test]
fn the_worktree_is_named_by_its_git_directory_alone() {
    // The main worktree has no name, its Git directory the common one.
    let main = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/\n");
    assert_eq!(main.git_directory(), Path::new("/w/repo/.git"));
    assert_eq!(main.worktree_name(), None);
    // A linked worktree's name is its Git directory's last component, whatever its
    // root is called: an ordinary repository's, a bare one's, and one whose name is
    // not UTF-8.
    for (answer, git_directory, name) in [
        (
            &b"/w/repo/.git/worktrees/agent\n/w/repo/.git\n/w/project-agent\n\n"[..],
            &b"/w/repo/.git/worktrees/agent"[..],
            &b"agent"[..],
        ),
        (
            b"/w/bare.git/worktrees/one\n/w/bare.git\n/w/elsewhere/two\nsub/\n",
            b"/w/bare.git/worktrees/one",
            b"one",
        ),
        (
            b"/w/repo/.git/worktrees/caf\xe9\xff\n/w/repo/.git\n/w/moved\n\n",
            b"/w/repo/.git/worktrees/caf\xe9\xff",
            b"caf\xe9\xff",
        ),
    ] {
        let workspace = read(answer);
        assert_eq!(
            workspace.git_directory(),
            Path::new(OsStr::from_bytes(git_directory)),
            "{}",
            answer.escape_ascii()
        );
        assert_eq!(
            workspace.worktree_name(),
            Some(OsStr::from_bytes(name)),
            "{}",
            answer.escape_ascii()
        );
        // The private repository is the Git directory's, which is named the same.
        assert_eq!(
            workspace.private_directory(),
            Path::new(OsStr::from_bytes(git_directory)).join("dupe")
        );
        assert!(workspace.linked());
    }
}

#[test]
fn attached_is_this_worktrees_private_directory_being_a_directory_by_lstat() {
    let scratch = env::temp_dir().join(format!("git-dupe-locate-{}", std::process::id()));
    let common = scratch.join("repo/.git");
    let linked = Workspace::linked_at(&common, b"one", &scratch.join("one"));
    let main = Workspace::at_root(&scratch.join("repo"));
    fs::create_dir_all(common.join("worktrees/one")).unwrap();
    // The main worktree attached leaves the linked one unattached.
    fs::create_dir(common.join("dupe")).unwrap();
    assert!(main.attached_now());
    assert!(!linked.attached_now());
    assert_eq!(linked.obstruction(), None);
    // A symbolic link at the linked worktree's own path is not its private repository,
    // even where it leads to one, and it obstructs that path; so does a file.
    std::os::unix::fs::symlink(common.join("dupe"), linked.private_directory()).unwrap();
    assert!(!linked.attached_now());
    assert_eq!(linked.obstruction(), Some(linked.private_directory()));
    fs::remove_file(linked.private_directory()).unwrap();
    fs::write(linked.private_directory(), b"").unwrap();
    assert_eq!(linked.obstruction(), Some(linked.private_directory()));
    fs::remove_file(linked.private_directory()).unwrap();
    fs::create_dir(linked.private_directory()).unwrap();
    assert!(linked.attached_now());
    assert_eq!(linked.obstruction(), None);
    fs::remove_dir(common.join("dupe")).unwrap();
    assert!(!main.attached_now());
    assert!(linked.attached_now());
    fs::remove_dir_all(&scratch).unwrap();
}

#[test]
fn the_private_directory_is_dupe_in_the_git_directory() {
    assert_eq!(
        private_directory(Path::new("/w/repo/.git")),
        Path::new("/w/repo/.git/dupe")
    );
}

/// A fresh root holding a file, a directory, a symbolic link to the directory, and a
/// dangling one, and nothing at `nothing`.
fn standing(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("git-dupe-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("directory")).unwrap();
    fs::write(root.join("file"), b"").unwrap();
    std::os::unix::fs::symlink("directory", root.join("link")).unwrap();
    std::os::unix::fs::symlink("nowhere", root.join("dangling")).unwrap();
    root
}

#[test]
fn present_is_whatever_lstat_finds_a_dangling_link_included() {
    let root = standing("present");
    for path in [&b"file"[..], b"directory", b"link", b"dangling"] {
        assert!(present(&root, path), "{:?}", OsStr::from_bytes(path));
    }
    for path in [&b"nothing"[..], b"file/below"] {
        assert!(!present(&root, path), "{:?}", OsStr::from_bytes(path));
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_directory_by_lstat_is_never_a_link_to_one() {
    let root = standing("directory-at");
    assert!(directory_at(&root, b"directory"));
    for path in [&b"file"[..], b"link", b"dangling", b"nothing"] {
        assert!(!directory_at(&root, path), "{:?}", OsStr::from_bytes(path));
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn paths_are_bytes() {
    let workspace = read(b"/w/caf\xe9/.git/worktrees/x\n/w/caf\xe9/.git\n/w/x\xff\n\n");
    assert_eq!(
        workspace.private_directory(),
        Path::new(OsStr::from_bytes(b"/w/caf\xe9/.git/worktrees/x/dupe"))
    );
    assert_eq!(workspace.root(), Path::new(OsStr::from_bytes(b"/w/x\xff")));
}

#[test]
fn a_prefix_holding_a_newline_is_the_rest_of_the_answer() {
    let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nnew\nline\n/sub/\n");
    assert_eq!(workspace.prefix(), b"new\nline\n/sub/");
    assert_eq!(workspace.git_directory, Path::new("/w/repo/.git"));
    assert_eq!(workspace.common_directory, Path::new("/w/repo/.git"));
    let workspace = read(b"/w/repo/.git/worktrees/l\n/w/repo/.git\n/w/l\nnew\nline\n/\n");
    assert_eq!(workspace.prefix(), b"new\nline\n/");
    assert_eq!(workspace.root(), Path::new("/w/l"));
}

#[test]
fn an_answer_of_another_shape_is_not_read() {
    for answer in [
        &b""[..],
        b"/w/repo/.git\n/w/repo/.git\n",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\n",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/",
        b"repo/.git\n/w/repo/.git\n/w/repo\n\n",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub\n",
        // A newline in one of the three paths, outside the envelope, as Git prints
        // it: at the root, and below it.
        b"/w/re\npo/.git\n/w/re\npo/.git\n/w/re\npo\n\n",
        b"/w/re\n/po/.git\n/w/re\n/po/.git\n/w/re\n/po\n\n",
        b"/w/re\n/po/.git\n/w/re\n/po/.git\n/w/re\n/po\nsub/\n",
        b"/w/repo/.git/worktrees/l\n/w/repo/.git\n/w/li\nnked\n\n",
    ] {
        for superproject in [false, true] {
            assert!(
                Workspace::read(answer, superproject).is_none(),
                "{}",
                answer.escape_ascii()
            );
        }
    }
}

#[test]
fn a_failed_locate_is_read_from_what_it_printed_before_failing() {
    assert!(matches!(Unlocated::read(b""), Some(Unlocated::Outside)));
    assert!(Unlocated::Outside.facts().is_none());
    // Inside a workspace's `.git` the two answers are the same directory; in a linked
    // worktree's Git directory the common one is the main repository's, whose private
    // repository is never the one used there: the linked worktree's own is.
    for (printed, private) in [
        (
            &b"/w/repo/.git\n/w/repo/.git\n"[..],
            &b"/w/repo/.git/dupe"[..],
        ),
        (b"/w/bare.git\n/w/bare.git\n", b"/w/bare.git/dupe"),
        (
            b"/w/repo/.git/worktrees/l\n/w/repo/.git\n",
            b"/w/repo/.git/worktrees/l/dupe",
        ),
        (b"/w/caf\xe9.git\n/w/caf\xe9.git\n", b"/w/caf\xe9.git/dupe"),
    ] {
        let Some(unlocated) = Unlocated::read(printed) else {
            panic!("{}", printed.escape_ascii());
        };
        let Unlocated::InGitDirectory {
            git_directory,
            common_directory,
        } = &unlocated
        else {
            panic!("{}", printed.escape_ascii());
        };
        assert_eq!(
            git_directory,
            Path::new(OsStr::from_bytes(private)),
            "{}",
            printed.escape_ascii()
        );
        // The facts keep the common Git directory the locate printed, and its parent
        // stands for the root.
        let facts = unlocated.facts().expect("facts inside a Git directory");
        assert_eq!(facts.common_directory(), common_directory);
        assert_eq!(Some(facts.root()), common_directory.parent());
        assert!(facts.outside().is_some());
    }
    for printed in [
        &b"/w/repo/.git\n"[..],
        b"/w/repo/.git\n/w/repo/.git",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\n",
        b".git\n.git\n",
        b"\n\n",
    ] {
        assert!(
            Unlocated::read(printed).is_none(),
            "{}",
            printed.escape_ascii()
        );
    }
}

#[test]
fn the_superproject_is_asked_immediately_before_the_prefix() {
    assert_eq!(words(false), LOCATE);
    assert_eq!(
        words(true),
        [
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
            "--show-superproject-working-tree",
            "--show-prefix",
        ]
    );
}

#[test]
fn asked_for_the_superproject_outside_a_submodule_the_answer_keeps_four_lines() {
    for answer in [
        &b"/w/repo/.git\n/w/repo/.git\n/w/repo\n\n"[..],
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/dir/\n",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\nnew\nline\n/\n",
    ] {
        let workspace = Workspace::read(answer, true).expect("four answers");
        assert!(!workspace.submodule_checkout(), "{}", answer.escape_ascii());
        assert_eq!(workspace.root(), Path::new("/w/repo"));
    }
}

#[test]
fn a_superproject_line_before_the_prefix_marks_a_submodule_checkout() {
    for (answer, root) in [
        (
            &b"/w/super/.git/modules/sub\n/w/super/.git/modules/sub\n/w/super/sub\n/w/super\n\n"[..],
            "/w/super/sub",
        ),
        (
            b"/w/s/.git/modules/m\n/w/s/.git/modules/m\n/w/s/m\n/w/s\nnew\nline\n/x/\n",
            "/w/s/m",
        ),
    ] {
        let workspace = Workspace::read(answer, true).expect("five answers");
        assert!(workspace.submodule_checkout(), "{}", answer.escape_ascii());
        assert_eq!(workspace.root(), Path::new(root));
        assert!(!workspace.prefix().starts_with(b"/"));
    }
    // Not asked, the same answer is of another shape; and a superproject line must
    // be followed by a prefix.
    let submodule =
        b"/w/super/.git/modules/sub\n/w/super/.git/modules/sub\n/w/super/sub\n/w/super\n\n";
    assert!(Workspace::read(submodule, false).is_none());
    assert!(Workspace::read(b"/w/r/.git\n/w/r/.git\n/w/r\n/w/super\n", true).is_none());
    assert!(Workspace::read(b"/w/r/.git\n/w/r/.git\n/w/r\n/w/s\n/x/\n", true).is_none());
}
