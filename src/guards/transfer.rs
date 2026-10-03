//! What `push`, `pull`, and `fetch` are compared by beyond their words (G18,
//! `Holds/G18`): whether the words give a repository, the URLs the private configuration
//! gives its remotes, and the default remote of a command given no repository. `places`
//! answers whether each names a public place; the front takes the configuration and the
//! branch, and writes the lines. The same records say whether `push` given no repository
//! leaves the choice of a remote to Git, which decides the front's hint after a failed
//! run (`Holds/G18`).
//!
//! What the code below cannot show:
//!
//! - **Given no repository** is read from the words left to right, up to the first `--`
//!   that is not presumed a value. A word that follows a dashed word other than one
//!   `known` lists for the command is presumed that word's value and is read as nothing
//!   else, a `--` and a `--repo` among them, because Git gives an option that requires a
//!   value the next word whatever it is (S13). Whether a word is presumed a value is
//!   decided by the word before it alone: a dashed word that is itself presumed a value
//!   presumes the next one too. A known word is the word exactly as written: a letter in
//!   a bundle, an abbreviation, a known word with a value after `=`, and a word of
//!   another command's list are not known, so that the word after each is presumed its
//!   value, as it is after a word a later Git adds. The default remote is then compared
//!   where Git would not consult it: over-inclusive, which refuses and sends nothing.
//! - A repository is given by a word after that `--`; by a word before it that is not
//!   dashed and not presumed a value; or, for `push`, by a `--repo` with a value,
//!   attached or as the next word, that is not itself presumed a value, unless a later
//!   word is `--no-repo` or a prefix of it at least as long as `--no-r`, which Git reads,
//!   or may come to read, as clearing it. `fetch` and `pull` have no `--repo`.
//! - A configuration key is matched as Git reads one: its section and variable without
//!   regard to ASCII case, the name between them exactly. Every `url` and `pushurl`
//!   record of every remote is a URL, whichever command runs, because Git pushes to every
//!   `url` of a remote and `fetch` asks a remote whose only URL is a `pushurl`. Of the
//!   three keys that give a default, the last record is the value; a record without a
//!   value, which Git refuses to read, is the empty value, which names the root.

/// The commands that move private history, each with its own reading of its words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    Push,
    Pull,
    Fetch,
}

/// The dashed words S13 records, for each command, as options that take no value or only
/// an attached one in every listed release, and so never the next word; `PULL` says why
/// it leaves out two.
const PUSH: [&[u8]; 30] = [
    b"-v",
    b"--verbose",
    b"-q",
    b"--quiet",
    b"--all",
    b"--branches",
    b"--mirror",
    b"-d",
    b"--delete",
    b"--tags",
    b"-n",
    b"--dry-run",
    b"--porcelain",
    b"-f",
    b"--force",
    b"--force-with-lease",
    b"--force-if-includes",
    b"--thin",
    b"-u",
    b"--set-upstream",
    b"--progress",
    b"--prune",
    b"--no-verify",
    b"--follow-tags",
    b"--signed",
    b"--atomic",
    b"-4",
    b"--ipv4",
    b"-6",
    b"--ipv6",
];

const FETCH: [&[u8]; 43] = [
    b"-v",
    b"--verbose",
    b"-q",
    b"--quiet",
    b"--all",
    b"--set-upstream",
    b"-a",
    b"--append",
    b"--atomic",
    b"-f",
    b"--force",
    b"-m",
    b"--multiple",
    b"-t",
    b"--tags",
    b"-n",
    b"--prefetch",
    b"-p",
    b"--prune",
    b"-P",
    b"--prune-tags",
    b"--recurse-submodules",
    b"--dry-run",
    b"--porcelain",
    b"--write-fetch-head",
    b"-k",
    b"--keep",
    b"-u",
    b"--update-head-ok",
    b"--progress",
    b"--unshallow",
    b"--refetch",
    b"--update-shallow",
    b"-4",
    b"--ipv4",
    b"-6",
    b"--ipv6",
    b"--negotiate-only",
    b"--auto-maintenance",
    b"--auto-gc",
    b"--show-forced-updates",
    b"--write-commit-graph",
    b"--stdin",
];

/// `pull` without `-j` and `--jobs`: it gives them no word of its own, but hands `--jobs`
/// to `fetch` ahead of the repository, and `fetch` takes the next word as its value, the
/// repository itself when nothing else `pull` passes on comes between, and then the
/// default remote; so the word after either is presumed a value, as for `fetch`.
const PULL: [&[u8]; 43] = [
    b"-v",
    b"--verbose",
    b"-q",
    b"--quiet",
    b"--progress",
    b"--recurse-submodules",
    b"-r",
    b"--rebase",
    b"-n",
    b"--stat",
    b"--log",
    b"--signoff",
    b"--squash",
    b"--commit",
    b"--edit",
    b"--ff",
    b"--ff-only",
    b"--verify",
    b"--verify-signatures",
    b"--autostash",
    b"-S",
    b"--gpg-sign",
    b"--allow-unrelated-histories",
    b"--all",
    b"-a",
    b"--append",
    b"-f",
    b"--force",
    b"-t",
    b"--tags",
    b"-p",
    b"--prune",
    b"--dry-run",
    b"-k",
    b"--keep",
    b"--unshallow",
    b"--update-shallow",
    b"-4",
    b"--ipv4",
    b"-6",
    b"--ipv6",
    b"--show-forced-updates",
    b"--set-upstream",
];

impl Transfer {
    /// The dashed words this command is known to give no separate word.
    fn known(self) -> &'static [&'static [u8]] {
        match self {
            Transfer::Push => &PUSH,
            Transfer::Pull => &PULL,
            Transfer::Fetch => &FETCH,
        }
    }
}

/// Whether `words`, those after the command, give `transfer` a repository; when they do
/// not, Git takes the default remote in its place.
pub fn gives_a_repository(transfer: Transfer, words: &[&[u8]]) -> bool {
    let known = transfer.known();
    let mut by_repo = false;
    let mut presumes_the_next = false;
    for (at, &word) in words.iter().enumerate() {
        let presumed = presumes_the_next;
        let dashed = word.starts_with(b"-");
        presumes_the_next = dashed && !known.contains(&word);
        if transfer == Transfer::Push && clears_repo(word) {
            by_repo = false;
        }
        if presumed {
            continue;
        }
        if word == b"--" {
            return at + 1 < words.len() || by_repo;
        }
        if !dashed {
            return true;
        }
        let repo = word.starts_with(b"--repo=") || (word == b"--repo" && at + 1 < words.len());
        if transfer == Transfer::Push && repo {
            by_repo = true;
        }
    }
    by_repo
}

/// Whether `push` reads `word` as `--no-repo`, or may come to: the word or a prefix of it
/// at least as long as `--no-r`.
fn clears_repo(word: &[u8]) -> bool {
    word.len() >= b"--no-r".len() && b"--no-repo".starts_with(word)
}

/// One record of the configuration: its key, and its value when it has one.
pub type Record<'a> = (&'a [u8], Option<&'a [u8]>);

/// Every URL the configuration's `records` give a remote, under `remote.<name>.url` or
/// `remote.<name>.pushurl`, each with the remote's name, in the records' order. A key
/// without a value gives no URL: Git refuses to read it as one.
pub fn remote_urls<'a>(records: &[Record<'a>]) -> Vec<(&'a [u8], &'a [u8])> {
    records
        .iter()
        .filter_map(|&(key, value)| {
            let key = Key::of(key)?;
            let url = key.variable.eq_ignore_ascii_case(b"url")
                || key.variable.eq_ignore_ascii_case(b"pushurl");
            if !key.section.eq_ignore_ascii_case(b"remote") || !url {
                return None;
            }
            Some((key.name?, value?))
        })
        .collect()
}

/// The remote a command given no repository takes in its place, as a value to compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefaultRemote<'a> {
    /// The key that gave it, as the configuration printed it.
    pub key: &'a [u8],
    pub value: &'a [u8],
}

/// The default remote of `transfer` given no repository, on `branch` or on a detached
/// `HEAD` when `None`: the first set of `branch.<branch>.pushRemote`,
/// `remote.pushDefault`, and `branch.<branch>.remote` for `push`, and
/// `branch.<branch>.remote` for `pull` and `fetch`. `None` when none is set, or when its
/// value is a remote with a `url`, whose URLs `remote_urls` gave already.
pub fn default_remote<'a>(
    transfer: Transfer,
    records: &[Record<'a>],
    branch: Option<&[u8]>,
) -> Option<DefaultRemote<'a>> {
    let keys = default_keys(transfer, branch);
    let found = keys.into_iter().find_map(|(section, name, variable)| {
        records
            .iter()
            .rev()
            .find(|&&(key, _)| is(key, section, name, variable))
            .map(|&(key, value)| DefaultRemote {
                key,
                value: value.unwrap_or_default(),
            })
    })?;
    let a_remote = records
        .iter()
        .any(|&(key, value)| value.is_some() && is(key, b"remote", Some(found.value), b"url"));
    (!a_remote).then_some(found)
}

/// Whether the configuration's `records` leave `push`, given no repository, on `branch`
/// or on a detached `HEAD` when `None`, to the remote Git takes of itself: none of its
/// three keys is reported, with a value or without, and `origin` has neither a `url` nor a
/// `pushurl`. Git then takes the only remote the configuration defines, when it defines
/// exactly one, and otherwise `origin`, which has no destination unless a file in
/// `remotes` or `branches` defines one (S13).
pub fn push_chooses_no_remote(records: &[Record<'_>], branch: Option<&[u8]>) -> bool {
    let reported = |section: &[u8], name: Option<&[u8]>, variable: &[u8]| {
        records
            .iter()
            .any(|&(key, _)| is(key, section, name, variable))
    };
    let origin = Some(b"origin".as_slice());
    !default_keys(Transfer::Push, branch)
        .into_iter()
        .any(|(section, name, variable)| reported(section, name, variable))
        && !reported(b"remote", origin, b"url")
        && !reported(b"remote", origin, b"pushurl")
}

/// The keys that give `transfer`, given no repository, its default remote, in the order
/// Git reads them, on `branch` or on a detached `HEAD` when `None`:
/// `branch.<branch>.pushRemote`, `remote.pushDefault`, and `branch.<branch>.remote` for
/// `push`, and `branch.<branch>.remote` for `pull` and `fetch`. A branch key names a
/// branch: on a detached `HEAD` there is none to read.
fn default_keys(transfer: Transfer, branch: Option<&[u8]>) -> Vec<KeyName<'_>> {
    let push_remote = (b"branch".as_slice(), branch, b"pushremote".as_slice());
    let push_default = (b"remote".as_slice(), None, b"pushdefault".as_slice());
    let remote = (b"branch".as_slice(), branch, b"remote".as_slice());
    let keys = match transfer {
        Transfer::Push => vec![push_remote, push_default, remote],
        Transfer::Pull | Transfer::Fetch => vec![remote],
    };
    keys.into_iter()
        .filter(|&(section, name, _)| name.is_some() || section == b"remote")
        .collect()
}

/// A key as `is` compares it: section, name, and variable.
type KeyName<'a> = (&'static [u8], Option<&'a [u8]>, &'static [u8]);

/// Whether `key` is `<section>.<name>.<variable>`, or `<section>.<variable>` when `name`
/// is `None`, as Git reads a key.
fn is(key: &[u8], section: &[u8], name: Option<&[u8]>, variable: &[u8]) -> bool {
    Key::of(key).is_some_and(|key| {
        key.section.eq_ignore_ascii_case(section)
            && key.variable.eq_ignore_ascii_case(variable)
            && key.name == name
    })
}

/// A configuration key's parts: the name is what lies between the first and the last
/// dot, and a key with one dot has none.
struct Key<'a> {
    section: &'a [u8],
    name: Option<&'a [u8]>,
    variable: &'a [u8],
}

impl<'a> Key<'a> {
    fn of(key: &'a [u8]) -> Option<Key<'a>> {
        let first = key.iter().position(|&byte| byte == b'.')?;
        let last = key.iter().rposition(|&byte| byte == b'.')?;
        Some(Key {
            section: &key[..first],
            name: (first != last).then(|| &key[first + 1..last]),
            variable: &key[last + 1..],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn given(transfer: Transfer, words: &str) -> bool {
        let words: Vec<&[u8]> = words.split(' ').map(str::as_bytes).collect();
        gives_a_repository(transfer, &words)
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
                assert!(!gives_a_repository(transfer, &words), "{words:?}");
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
            // The `--` after `--repo=origin` is presumed its value, so a `--no-repo` after
            // it is a later word that clears it, where Git reads a refspec.
            "--repo=origin -- --no-repo",
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
}
