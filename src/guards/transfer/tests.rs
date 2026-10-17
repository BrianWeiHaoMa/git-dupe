use super::*;

fn read(transfer: Transfer, words: &str) -> Repository<'_> {
    let words: Vec<&[u8]> = words.split(' ').map(str::as_bytes).collect();
    repository(transfer, &words)
}

fn given(transfer: Transfer, words: &str) -> bool {
    read(transfer, words) != Repository::Default
}

#[test]
fn a_word_that_is_neither_dashed_nor_a_value_gives_a_repository() {
    for words in [
        "origin",
        "origin main",
        "-u origin main",
        "-u origin",
        "--set-upstream origin",
        "-o ci.skip origin",
        "--push-option ci.skip origin",
        "-- origin main",
        "-q -- origin",
        "-uq origin main",
    ] {
        assert!(given(Transfer::Push, words), "push {words}");
    }
    for words in ["origin", "-v origin", "--prune origin", "-m one two"] {
        assert!(given(Transfer::Fetch, words), "fetch {words}");
        assert!(given(Transfer::Pull, words), "pull {words}");
    }
    assert!(given(Transfer::Pull, "--rebase origin"));
}

#[test]
fn a_command_with_no_such_word_is_given_no_repository() {
    for words in ["", "-q", "-u", "--all", "--tags --prune", "--"] {
        let words: Vec<&[u8]> = if words.is_empty() {
            Vec::new()
        } else {
            words.split(' ').map(str::as_bytes).collect()
        };
        for transfer in [Transfer::Push, Transfer::Pull, Transfer::Fetch] {
            assert_eq!(
                repository(transfer, &words),
                Repository::Default,
                "{words:?}"
            );
        }
    }
}

/// The forms where a looser reading takes a value of an option for the repository,
/// and Git sends to the default remote (`Holds/G18`).
#[test]
fn a_word_after_a_dashed_word_not_known_is_its_value_whatever_it_is() {
    for words in [
        // A `--repo` Git reads as the value of the option before it.
        "-o --repo=origin",
        "--push-option --repo=origin",
        "-qo --repo=origin",
        // A word after a `--` that is itself such a value.
        "-o -- -q",
        "-o -- origin",
        // A `--repo` a later word clears.
        "--repo=origin --no-repo",
        "--repo=origin --no-rep",
        "--repo=origin --no-r",
        "--repo origin --no-repo",
        "--repo=origin -o --no-repo",
        // `--no-repo` is not known, so the `--repo` after it is presumed its value.
        "--no-repo --repo=origin",
        // A dashed word presumed a value presumes the next.
        "--no-thin -o origin",
        "--force-with-lease=main -o origin",
        "-o -o origin",
        // A word not known: an abbreviation, a bundle, a later Git's option, help.
        "--verb origin",
        "-vu origin",
        "--later-option origin",
        "-h origin",
        "--help origin",
        // `--repo` with no value.
        "--repo",
        "--rep=origin",
    ] {
        assert!(!given(Transfer::Push, words), "push {words}");
    }
    for words in [
        "-o -- -q",
        "--repo=origin",
        "--repo origin",
        "-o -h",
        "--server-option --repo=origin",
    ] {
        assert!(!given(Transfer::Fetch, words), "fetch {words}");
        assert!(!given(Transfer::Pull, words), "pull {words}");
    }
    // Each list is the command's own: `--jobs` takes the next word in `fetch`.
    assert!(!given(Transfer::Fetch, "--jobs origin"));
    assert!(!given(Transfer::Fetch, "-j origin"));
    assert!(!given(Transfer::Push, "--rebase origin"));
    // `pull` takes no word for `-j` itself but hands `--jobs` to `fetch` ahead of the
    // repository, so `fetch` can read that word as its value and take the default.
    for words in ["-j 1", "--jobs 1", "-j origin", "--jobs origin"] {
        assert!(!given(Transfer::Pull, words), "pull {words}");
    }
    assert!(given(Transfer::Pull, "--jobs 1 origin"));
}

#[test]
fn push_is_given_a_repository_by_a_repo_with_a_value() {
    for words in [
        "--repo=origin",
        "--repo origin",
        "--repo=",
        "--repo=origin --no-thin",
        "--repo=origin --no-repository",
        "--repo=origin --no-",
        "--repo -- main",
        "--repo=origin --",
    ] {
        assert!(given(Transfer::Push, words), "push {words}");
    }
}

/// The repository word is the first word that is no option, as Git takes it, else a
/// standing `--repo`'s value; a later such word is a refspec, an option or a value is not.
#[test]
fn the_repository_word_stands_alone_until_a_refspec_follows_it() {
    for (words, word) in [
        ("origin", "origin"),
        ("-v origin", "origin"),
        ("origin -n", "origin"),
        ("origin -o --help", "origin"),
        ("-- origin", "origin"),
        ("origin --", "origin"),
        ("--repo origin", "origin"),
        ("--repo=origin", "origin"),
        ("--repo origin --", "origin"),
        // Git pushes to the word, whatever `--repo` says, before or after it.
        ("--repo origin backup", "backup"),
        ("backup --repo=origin", "backup"),
        // A word presumed a value is no refspec.
        ("-o main origin", "origin"),
        // A word holding `=` takes its value attached and no next word, so the word after
        // it is the repository, as Git takes it whatever the `--repo` before it said.
        ("--repo=origin main", "main"),
        ("--recurse-submodules=check origin", "origin"),
        ("-o=ci.skip origin", "origin"),
        ("-o --force-with-lease=main origin", "origin"),
        // After a `--` that is no value, the first word, whatever it spells.
        ("--repo=origin -- --no-repo", "--no-repo"),
    ] {
        let expected = Repository::Alone(word.as_bytes());
        assert_eq!(read(Transfer::Push, words), expected, "push {words}");
    }
    // With a refspec after it, the repository word is still the one Git pushes to.
    for (words, word) in [
        ("origin main", "origin"),
        ("origin HEAD", "origin"),
        ("-- origin main", "origin"),
        ("origin -- main", "origin"),
        ("--all origin main", "origin"),
        ("--repo=backup origin main", "origin"),
        ("--recurse-submodules=check origin main", "origin"),
        ("-v -- backup main dest", "backup"),
    ] {
        let expected = Repository::WithRefspec(word.as_bytes());
        assert_eq!(read(Transfer::Push, words), expected, "push {words}");
        assert_eq!(expected.word(), Some(word.as_bytes()));
    }
    for words in ["origin main", "-- origin main", "origin -- main"] {
        assert_eq!(
            read(Transfer::Pull, words),
            Repository::WithRefspec(b"origin"),
            "pull {words}"
        );
    }
    // Without `=`, a word not known still takes the next as its value.
    assert_eq!(
        read(Transfer::Push, "--recurse-submodules origin main"),
        Repository::Alone(b"main")
    );
    assert_eq!(
        read(Transfer::Push, "--all origin"),
        Repository::Alone(b"origin")
    );
    assert_eq!(Repository::Alone(b"origin").word(), Some(&b"origin"[..]));
    assert_eq!(Repository::Default.word(), None);
    for words in ["origin", "--rebase origin", "origin -v", "--jobs 1 origin"] {
        assert_eq!(
            read(Transfer::Pull, words),
            Repository::Alone(b"origin"),
            "pull {words}"
        );
    }
    // A cleared `--repo`, and one `pull` does not have, give none.
    assert_eq!(
        read(Transfer::Push, "--repo=origin --no-repo"),
        Repository::Default
    );
    assert_eq!(read(Transfer::Pull, "--repo=origin"), Repository::Default);
}

#[test]
fn a_given_remote_is_the_branchs_remote_else_origin_for_push_alone() {
    let is = |listing: &[(&str, Option<&str>)], transfer, word: &str| {
        is_the_branch_remote(transfer, &records(listing), b"main", word.as_bytes())
    };
    let two = [
        ("remote.backup.url", Some("/b")),
        ("remote.other.url", Some("/o")),
    ];
    let keyed = [("branch.main.remote", Some("backup"))];
    for transfer in [Transfer::Push, Transfer::Pull] {
        assert!(is(&keyed, transfer, "backup"));
        assert!(!is(&keyed, transfer, "origin"));
        assert!(!is(&keyed, transfer, "Backup"));
        // The last record is the value.
        let twice = [keyed[0], ("Branch.main.REMOTE", Some("origin"))];
        assert!(is(&twice, transfer, "origin"));
        // Another branch's key is not this one's.
        let other = [two[0], two[1], ("branch.other.remote", Some("backup"))];
        assert!(!is(&other, transfer, "backup"));
    }
    // With the key unset and two remotes defined, the word counted, `push` compares
    // `origin`, whatever the push keys hold; `pull` compares nothing.
    for push_key in [
        None,
        Some(("remote.pushdefault", Some("backup"))),
        Some(("branch.main.pushremote", Some("backup"))),
    ] {
        let listing: Vec<_> = two.into_iter().chain(push_key).collect();
        assert!(is(&listing, Transfer::Push, "origin"), "{listing:?}");
        assert!(!is(&listing, Transfer::Push, "backup"), "{listing:?}");
        assert!(!is(&listing, Transfer::Pull, "origin"), "{listing:?}");
        assert!(!is(&listing, Transfer::Pull, "backup"), "{listing:?}");
    }
}

#[test]
fn with_the_key_unset_push_compares_the_one_remote_defined_the_word_counted() {
    let is = |listing: &[(&str, Option<&str>)], transfer, word: &str| {
        is_the_branch_remote(transfer, &records(listing), b"main", word.as_bytes())
    };
    // No remote defined: the word is the one; the push keys define none.
    for listing in [
        &[][..],
        &[("remote.pushdefault", Some("backup"))],
        &[("branch.main.pushremote", Some("backup"))],
        &[("remotes.group", Some("backup other"))],
    ] {
        for word in ["backup", "nosuch", "/a/path", "origin"] {
            assert!(is(listing, Transfer::Push, word), "{listing:?} {word}");
        }
        assert!(!is(listing, Transfer::Pull, "backup"), "{listing:?}");
    }
    // One remote defined, by any of its keys, with a value or without: that one, and
    // `origin` once another word makes two.
    for key in [
        "remote.backup.url",
        "Remote.backup.VCS",
        "remote.backup.fetch",
    ] {
        for value in [Some("/b"), None] {
            let listing = [(key, value)];
            assert!(is(&listing, Transfer::Push, "backup"), "{listing:?}");
            assert!(is(&listing, Transfer::Push, "origin"), "{listing:?}");
            assert!(!is(&listing, Transfer::Push, "Backup"), "{listing:?}");
            assert!(!is(&listing, Transfer::Push, "other"), "{listing:?}");
        }
    }
    // A name holding a dot is one name.
    let dotted = [("remote.back.up.url", Some("/b"))];
    assert!(is(&dotted, Transfer::Push, "back.up"));
    assert!(!is(&dotted, Transfer::Push, "back"));
    // Two keys of one remote define one; two remotes, `origin` alone.
    let one = [
        ("remote.backup.url", Some("/b")),
        (
            "remote.backup.fetch",
            Some("+refs/heads/*:refs/remotes/backup/*"),
        ),
    ];
    assert!(is(&one, Transfer::Push, "backup"));
    let with_origin = [one[0], ("remote.origin.fetch", Some("x"))];
    assert!(!is(&with_origin, Transfer::Push, "backup"));
    assert!(is(&with_origin, Transfer::Push, "origin"));
    // A name beginning with `/` defines none; an empty name defines `.<variable>`.
    let slashed = [("remote./x.url", Some("/b"))];
    assert!(is(&slashed, Transfer::Push, "backup"));
    let empty = [("remote..url", Some("/b"))];
    assert!(!is(&empty, Transfer::Push, "backup"));
    assert!(is(&empty, Transfer::Push, ".url"));
    assert!(!is(&empty, Transfer::Push, ".fetch"));
    assert!(!is(&empty, Transfer::Push, "url"));
    // The branch's key, where set, decides alone.
    let keyed = [one[0], ("branch.main.remote", Some("other"))];
    assert!(!is(&keyed, Transfer::Push, "backup"));
    assert!(!is(&keyed, Transfer::Push, "origin"));
    assert!(is(&keyed, Transfer::Push, "other"));
}

fn records<'a>(listing: &[(&'a str, Option<&'a str>)]) -> Vec<Record<'a>> {
    listing
        .iter()
        .map(|&(key, value)| (key.as_bytes(), value.map(str::as_bytes)))
        .collect()
}

#[test]
fn every_url_and_push_url_of_every_remote_is_compared() {
    let listing = records(&[
        ("core.bare", Some("false")),
        ("remote.origin.url", Some("/srv/private.git")),
        (
            "remote.origin.fetch",
            Some("+refs/heads/*:refs/remotes/origin/*"),
        ),
        ("remote.two.url", Some("/srv/first.git")),
        ("remote.two.url", Some("https://example.com/team/project")),
        ("remote.push.pushurl", Some("..")),
        ("Remote.Dotted.Name.URL", Some("")),
        ("remote.novalue.url", None),
        ("remote.url", Some("/nowhere")),
        ("branch.main.url", Some("/nowhere")),
    ]);
    let urls: Vec<(&[u8], &[u8])> = remote_urls(&listing);
    assert_eq!(
        urls,
        [
            (&b"origin"[..], &b"/srv/private.git"[..]),
            (b"two", b"/srv/first.git"),
            (b"two", b"https://example.com/team/project"),
            (b"push", b".."),
            (b"Dotted.Name", b""),
        ]
    );
}

#[test]
fn every_word_of_every_remote_group_is_a_member() {
    let listing = records(&[
        ("remotes.g", Some("origin /srv/root")),
        ("remote.g.url", Some("/srv/private.git")),
        ("remotes.ws", Some(" a\tb\n c\r ")),
        ("Remotes.Dotted.Name", Some("..")),
        ("remotes.empty", Some("")),
        ("remotes.novalue", None),
        ("remotes", Some("/nowhere")),
        ("remote.remotes", Some("/nowhere")),
    ]);
    assert_eq!(
        group_members(&listing),
        [
            (&b"remotes.g"[..], &b"origin"[..]),
            (b"remotes.g", b"/srv/root"),
            (b"remotes.ws", b"a"),
            (b"remotes.ws", b"b"),
            (b"remotes.ws", b"c\r"),
            (b"Remotes.Dotted.Name", b".."),
        ]
    );
}

fn default_of<'a>(
    transfer: Transfer,
    listing: &[Record<'a>],
    branch: Option<&str>,
) -> Option<(&'a str, &'a str)> {
    default_remote(transfer, listing, branch.map(str::as_bytes)).map(|found| {
        (
            std::str::from_utf8(found.key).unwrap(),
            std::str::from_utf8(found.value).unwrap(),
        )
    })
}

#[test]
fn push_takes_the_first_set_of_its_three_keys() {
    let all = records(&[
        ("branch.main.remote", Some(".")),
        ("remote.pushdefault", Some("/srv/default")),
        ("branch.main.pushremote", Some("/srv/push")),
    ]);
    let main = Some("main");
    assert_eq!(
        default_of(Transfer::Push, &all, main),
        Some(("branch.main.pushremote", "/srv/push"))
    );
    assert_eq!(
        default_of(Transfer::Push, &all[..2], main),
        Some(("remote.pushdefault", "/srv/default"))
    );
    assert_eq!(
        default_of(Transfer::Push, &all[..1], main),
        Some(("branch.main.remote", "."))
    );
    // `pull` and `fetch` read `branch.<name>.remote` alone.
    for transfer in [Transfer::Pull, Transfer::Fetch] {
        assert_eq!(
            default_of(transfer, &all, main),
            Some(("branch.main.remote", "."))
        );
        assert_eq!(default_of(transfer, &all[1..], main), None);
    }
    // On a detached `HEAD` the branch keys are not read.
    assert_eq!(
        default_of(Transfer::Push, &all, None),
        Some(("remote.pushdefault", "/srv/default"))
    );
    assert_eq!(default_of(Transfer::Fetch, &all, None), None);
    // Another branch's keys are not this one's.
    assert_eq!(default_of(Transfer::Fetch, &all, Some("Main")), None);
    assert_eq!(default_of(Transfer::Push, &[], main), None);
}

#[test]
fn a_key_is_read_as_git_reads_it_and_the_last_record_is_its_value() {
    let listing = records(&[
        ("Branch.main.REMOTE", Some("/srv/first")),
        ("branch.main.remote", Some("/srv/last")),
        ("branch.feature.x.remote", Some("/srv/dotted")),
        ("branch.empty.remote", Some("")),
        ("branch.bare.remote", None),
    ]);
    assert_eq!(
        default_of(Transfer::Fetch, &listing, Some("main")),
        Some(("branch.main.remote", "/srv/last"))
    );
    assert_eq!(
        default_of(Transfer::Pull, &listing, Some("feature.x")),
        Some(("branch.feature.x.remote", "/srv/dotted"))
    );
    // An empty value, and a key without one, are the empty path: the root.
    assert_eq!(
        default_of(Transfer::Fetch, &listing, Some("empty")),
        Some(("branch.empty.remote", ""))
    );
    assert_eq!(
        default_of(Transfer::Fetch, &listing, Some("bare")),
        Some(("branch.bare.remote", ""))
    );
}

#[test]
fn push_chooses_no_remote_while_no_key_and_no_url_of_origin_is_reported() {
    let main = Some(b"main".as_slice());
    let chooses_none = |listing: &[(&str, Option<&str>)], branch: Option<&[u8]>| {
        push_chooses_no_remote(&records(listing), branch)
    };
    for listing in [
        &[][..],
        &[("core.bare", Some("false"))],
        // Remotes of other names, and a key of `origin` that gives no URL.
        &[("remote.backup.url", Some("/srv/backup.git"))],
        &[
            ("remote.backup.url", Some("/srv/backup.git")),
            ("remote.other.url", Some("/srv/other.git")),
        ],
        &[(
            "remote.origin.fetch",
            Some("+refs/heads/*:refs/remotes/origin/*"),
        )],
        // A remote's name is compared exactly.
        &[("remote.Origin.url", Some("/srv/private.git"))],
        // Another branch's keys are not this one's.
        &[("branch.other.remote", Some("backup"))],
        &[("branch.Main.pushremote", Some("backup"))],
    ] {
        assert!(chooses_none(listing, main), "{listing:?}");
        assert!(chooses_none(listing, None), "{listing:?} detached");
    }
    for listing in [
        &[("branch.main.pushremote", Some("backup"))][..],
        &[("remote.pushdefault", Some("backup"))],
        &[("branch.main.remote", Some("backup"))],
        // Each key is reported with a value or without, in any case of its section
        // and variable.
        &[("branch.main.pushremote", None)],
        &[("remote.pushdefault", None)],
        &[("branch.main.remote", None)],
        &[("BRANCH.main.PushRemote", Some("backup"))],
        &[("Remote.PushDefault", Some(""))],
        &[("remote.origin.url", Some("/srv/private.git"))],
        &[("remote.origin.pushurl", Some("/srv/private.git"))],
        &[("remote.origin.url", None)],
        &[("Remote.origin.PUSHURL", Some("/srv/private.git"))],
        // A key naming a remote that has a URL chooses it, though `default_remote`
        // answers `None` for it, its URLs already compared.
        &[
            ("remote.backup.url", Some("/srv/backup.git")),
            ("branch.main.pushremote", Some("backup")),
        ],
    ] {
        assert!(!chooses_none(listing, main), "{listing:?}");
    }
    // On a detached `HEAD` the branch keys are not read; `remote.pushDefault` is.
    for listing in [
        &[("branch.main.pushremote", Some("backup"))][..],
        &[("branch.main.remote", None)],
    ] {
        assert!(chooses_none(listing, None), "{listing:?} detached");
    }
    assert!(!chooses_none(
        &[("remote.pushdefault", Some("backup"))],
        None
    ));
    assert!(!chooses_none(
        &[("remote.origin.url", Some("/srv/o.git"))],
        None
    ));
}

#[test]
fn a_default_that_is_a_remote_with_a_url_is_not_compared_again() {
    let listing = records(&[
        ("branch.main.remote", Some("origin")),
        ("remote.origin.url", Some("/srv/private.git")),
        ("branch.side.remote", Some("pushonly")),
        ("remote.pushonly.pushurl", Some("/srv/push.git")),
        ("branch.dot.remote", Some(".")),
        ("remote...url", Some("/srv/dot.git")),
        ("branch.none.remote", Some("novalue")),
        ("remote.novalue.url", None),
        ("branch.case.remote", Some("Origin")),
    ]);
    assert_eq!(default_of(Transfer::Fetch, &listing, Some("main")), None);
    assert_eq!(default_of(Transfer::Fetch, &listing, Some("dot")), None);
    // A `pushurl` alone does not make the value a remote: it is used as a URL.
    assert_eq!(
        default_of(Transfer::Fetch, &listing, Some("side")),
        Some(("branch.side.remote", "pushonly"))
    );
    assert_eq!(
        default_of(Transfer::Fetch, &listing, Some("none")),
        Some(("branch.none.remote", "novalue"))
    );
    // A remote's name is compared exactly.
    assert_eq!(
        default_of(Transfer::Fetch, &listing, Some("case")),
        Some(("branch.case.remote", "Origin"))
    );
}

#[test]
fn push_takes_the_remote_of_its_first_key_reported_and_otherwise_origin() {
    let main = Some(b"main".as_slice());
    let remote = |listing: &[(&str, Option<&str>)], branch: Option<&[u8]>| {
        std::str::from_utf8(push_remote(&records(listing), branch))
            .unwrap()
            .to_owned()
    };
    let all = [
        ("branch.main.remote", Some("fetched")),
        ("remote.pushdefault", Some("default")),
        ("branch.main.pushremote", Some("pushed")),
        ("remote.origin.url", Some("/srv/o.git")),
    ];
    assert_eq!(remote(&all, main), "pushed");
    assert_eq!(remote(&all[..2], main), "default");
    assert_eq!(remote(&all[..1], main), "fetched");
    // A remote the configuration defines a URL for is named, as is a URL.
    assert_eq!(remote(&all[3..], main), "origin");
    assert_eq!(
        remote(&[("branch.main.remote", Some("/srv/b.git"))], main),
        "/srv/b.git"
    );
    // The last record is the value; a record without one is the empty value.
    assert_eq!(
        remote(
            &[
                ("branch.main.pushremote", Some("first")),
                ("Branch.main.PushRemote", Some("last")),
            ],
            main
        ),
        "last"
    );
    assert_eq!(remote(&[("remote.pushdefault", None)], main), "");
    // Another branch's keys, and on a detached `HEAD` every branch key, are not read.
    assert_eq!(
        remote(&[("branch.other.pushremote", Some("x"))], main),
        "origin"
    );
    assert_eq!(remote(&all[..1], None), "origin");
    assert_eq!(remote(&all, None), "default");
}

#[test]
fn a_branch_has_an_upstream_while_its_merge_key_and_for_push_its_remote_are_reported() {
    let has = |transfer, listing: &[(&str, Option<&str>)], branch: &str| {
        has_upstream(transfer, &records(listing), branch.as_bytes())
    };
    let remote = ("branch.main.remote", Some("origin"));
    for merge in [
        ("branch.main.merge", Some("refs/heads/main")),
        ("BRANCH.main.MERGE", Some("refs/heads/main")),
        ("branch.main.merge", None),
    ] {
        assert!(has(Transfer::Pull, &[merge], "main"), "{merge:?}");
        assert!(has(Transfer::Push, &[merge, remote], "main"), "{merge:?}");
        assert!(has(Transfer::Push, &[remote, merge], "main"), "{merge:?}");
        assert!(
            has(
                Transfer::Push,
                &[merge, ("BRANCH.main.REMOTE", None)],
                "main"
            ),
            "{merge:?}"
        );
        // `push` counts no upstream while either key is unset, where `pull` reads `merge`.
        assert!(!has(Transfer::Push, &[merge], "main"), "{merge:?}");
        assert!(
            !has(
                Transfer::Push,
                &[merge, ("branch.other.remote", Some("origin"))],
                "main"
            ),
            "{merge:?}"
        );
    }
    for listing in [
        &[][..],
        &[remote],
        &[("branch.Main.merge", Some("refs/heads/main")), remote],
        &[("branch.other.merge", Some("refs/heads/main")), remote],
        &[("branch.merge", Some("refs/heads/main")), remote],
        &[("branch.main.merge.x", Some("refs/heads/main")), remote],
    ] {
        for transfer in [Transfer::Push, Transfer::Pull] {
            assert!(!has(transfer, listing, "main"), "{transfer:?} {listing:?}");
        }
    }
}

/// Git compares the raw value of the one `merge` with `refs/heads/<name>`, and names the
/// remote of `branch.<name>.remote` (S13).
#[test]
fn an_upstream_of_another_name_is_the_one_merge_value_other_than_the_branchs_ref() {
    let upstream = |listing: &[(&str, Option<&str>)]| {
        upstream_of_another_name(&records(listing), b"topic")
            .map(|upstream| (upstream.remote.to_vec(), upstream.merge.to_vec()))
    };
    let remote = ("branch.topic.remote", Some("origin"));
    for (merge, expected) in [
        ("refs/heads/main", "refs/heads/main"),
        // Without the prefix, the branch's own name is another to Git.
        ("topic", "topic"),
        ("refs/tags/topic", "refs/tags/topic"),
        ("refs/heads/Topic", "refs/heads/Topic"),
    ] {
        let found = upstream(&[remote, ("branch.topic.merge", Some(merge))]);
        assert_eq!(
            found,
            Some((b"origin".to_vec(), expected.as_bytes().to_vec())),
            "{merge}"
        );
    }
    // The remote is the last record's, as Git reads it.
    let found = upstream(&[
        remote,
        ("branch.topic.merge", Some("refs/heads/main")),
        ("branch.topic.remote", Some("backup")),
    ]);
    assert_eq!(found.map(|(remote, _)| remote), Some(b"backup".to_vec()));
    for listing in [
        // The branch's own ref: an upstream of its name.
        &[remote, ("branch.topic.merge", Some("refs/heads/topic"))][..],
        // No upstream for `push`: the no-upstream hint's, not this one's.
        &[("branch.topic.merge", Some("refs/heads/main"))],
        &[remote],
        // Several values, which Git refuses naming no command; one it cannot read.
        &[
            remote,
            ("branch.topic.merge", Some("refs/heads/main")),
            ("branch.topic.merge", Some("refs/heads/other")),
        ],
        &[
            remote,
            ("branch.topic.merge", Some("refs/heads/main")),
            ("branch.topic.merge", None),
        ],
        &[remote, ("branch.topic.merge", None)],
        // Another branch's keys.
        &[
            ("branch.main.remote", Some("origin")),
            ("branch.main.merge", Some("refs/heads/other")),
        ],
    ] {
        assert_eq!(upstream(listing), None, "{listing:?}");
    }
}

#[test]
fn push_default_is_simple_while_unset_or_its_last_record_says_simple_exactly() {
    let simple = |listing: &[(&str, Option<&str>)]| push_default_is_simple(&records(listing));
    for listing in [
        &[][..],
        &[("push.default", Some("simple"))],
        &[("PUSH.Default", Some("simple"))],
        &[
            ("push.default", Some("current")),
            ("push.default", Some("simple")),
        ],
        &[("push.autoSetupRemote", Some("true"))],
        &[("remote.push.default", Some("current"))],
    ] {
        assert!(simple(listing), "{listing:?}");
    }
    for listing in [
        &[("push.default", Some("current"))][..],
        &[("push.default", Some("upstream"))],
        &[("push.default", Some("tracking"))],
        &[("push.default", Some("matching"))],
        &[("push.default", Some("nothing"))],
        &[("push.default", Some("Simple"))],
        &[("push.default", None)],
        &[
            ("push.default", Some("simple")),
            ("push.default", Some("current")),
        ],
    ] {
        assert!(!simple(listing), "{listing:?}");
    }
}

#[test]
fn a_reconciliation_is_configured_by_pull_rebase_the_branchs_rebase_or_pull_ff() {
    let main = Current::Branch(b"main");
    let configured = |listing: &[(&str, Option<&str>)], current: Current<'_>| {
        reconciliation_configured(&records(listing), current)
    };
    for listing in [
        &[("pull.rebase", Some("false"))][..],
        &[("pull.rebase", None)],
        &[("Pull.Rebase", Some("merges"))],
        &[("pull.ff", Some("only"))],
        &[("pull.ff", None)],
        &[("PULL.FF", Some("false"))],
    ] {
        for current in [main, Current::Detached, Current::AnyBranch] {
            assert!(configured(listing, current), "{listing:?} {current:?}");
        }
    }
    let rebase_of_main = [("branch.main.rebase", Some("true"))];
    assert!(configured(&rebase_of_main, main));
    assert!(configured(&[("Branch.main.REBASE", None)], main));
    // A detached `HEAD` reads no branch's key.
    assert!(!configured(&rebase_of_main, Current::Detached));
    // Another branch's key, and a name compared exactly, are not this branch's.
    let rebase_of_other = [("branch.other.rebase", Some("true"))];
    assert!(!configured(&rebase_of_other, main));
    assert!(!configured(&[("branch.Main.rebase", Some("true"))], main));
    // Where `HEAD` was not read, any branch's key counts.
    assert!(configured(&rebase_of_other, Current::AnyBranch));
    assert!(configured(&rebase_of_main, Current::AnyBranch));
    for listing in [
        &[][..],
        &[("branch.main.merge", Some("refs/heads/main"))],
        &[("pull.twohead", Some("ort"))],
        &[("pull.rebase.x", Some("true"))],
        &[("branch.rebase", Some("true"))],
        &[("rebase.autoStash", Some("true"))],
    ] {
        for current in [main, Current::Detached, Current::AnyBranch] {
            assert!(!configured(listing, current), "{listing:?} {current:?}");
        }
    }
}
