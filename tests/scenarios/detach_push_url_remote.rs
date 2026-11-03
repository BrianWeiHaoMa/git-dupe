//! What counts as a configured private remote when `git dupe detach` decides whether a
//! commit would be lost (G3): a remote Git lists, its `pushurl` set and no `url`, is one.

use crate::harness::{
    End, Scenario, Transfer, Tree, detached, dupe_succeeds, names, refusal, under_each_release,
};

/// The transfer workspace `name` whose private remote `origin` has a `pushurl` naming its
/// private bare repository and a fetch refspec, and no `url`.
fn push_url_workspace(s: &Scenario, name: &str) -> Transfer {
    let transfer = s.transfer_workspace(name);
    let private_remote = transfer.private_remote.to_str().expect("a UTF-8 path");
    dupe_succeeds(
        s,
        &transfer.root,
        &["config", "remote.origin.pushurl", private_remote],
    );
    dupe_succeeds(
        s,
        &transfer.root,
        &[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
    );
    let url = s
        .private(&transfer.root)
        .git(["config", "--get", "remote.origin.url"])
        .run();
    assert_eq!(url.end, End::Code(1), "{url:?}");
    transfer
}

/// Unpushed, the commit is refused as on no remote-tracking branch of a configured remote;
/// pushed there, `detach` removes the private repository and the region, keeping every
/// file of the working tree.
#[test]
fn a_remote_with_a_push_url_alone_is_configured() {
    under_each_release(|s| {
        let transfer = push_url_workspace(s, "push-url");
        let root = &transfer.root;

        let unpushed = s.git(["dupe", "detach"]).from(root).run();
        let line = refusal(&unpushed, &[b"git dupe detach --force"]);
        names(line, b"no remote-tracking branch");
        assert!(root.join(".git/dupe").is_dir());

        dupe_succeeds(s, root, &["push", "-q", "origin", "main"]);
        s.private(root)
            .git(["show-ref", "--verify", "-q", "refs/remotes/origin/main"])
            .succeeds();
        let before = Tree::working(root);
        dupe_succeeds(s, root, &["detach"]);
        detached(root);
        let changed = before.changed_in(&Tree::working(root));
        assert!(changed.is_empty(), "changed: {changed:?}");
    });
}
