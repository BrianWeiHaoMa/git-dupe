//! What `push`, `pull`, and `fetch` are compared by beyond their words (G18,
//! `Holds/G18`): whether the words give a repository, the URLs the private configuration
//! gives its remotes, the members of its remote groups, and the default remote of a
//! command given no repository. `places`
//! answers whether each names a public place; the front takes the configuration and the
//! branch, and writes the lines. The same words and records decide the front's hints
//! after a failed run (`Holds/G18`): whether `push` given no repository leaves the choice
//! of a remote to Git, which remote it takes, whether a repository given with no refspec
//! is the one Git's advice names a plain command for, whether a branch has an upstream,
//! whether that upstream has another name and `push.default` has Git refuse for it, and
//! whether `pull` is told how to reconcile divergent branches.
//!
//! What the code below cannot show:
//!
//! - **Given no repository** is read from the words left to right, up to the first `--`
//!   that is not presumed a value. A word that follows a dashed word other than one
//!   `known` lists for the command, and other than one holding `=`, is presumed that
//!   word's value and is read as nothing else, a `--` and a `--repo` among them, because
//!   Git gives an option that requires a value the next word whatever it is, and none to
//!   a word whose value is attached after its `=` (S13). Whether a word is presumed a
//!   value is decided by the word before it alone: a dashed word that is itself presumed
//!   a value presumes the next one too, unless it holds `=`. A known word is the word
//!   exactly as written: a letter in a bundle, an abbreviation, and a word of another
//!   command's list are not known, so that the word after each is presumed its value, as
//!   it is after a word a later Git adds. The default remote is then compared where Git
//!   would not consult it: over-inclusive, which refuses and sends nothing.
//! - A repository is given by a word after that `--`; by a word before it that is not
//!   dashed and not presumed a value; or, for `push`, by a `--repo` with a value,
//!   attached or as the next word, that is not itself presumed a value, unless a later
//!   word is `--no-repo` or a prefix of it at least as long as `--no-r`, which Git reads,
//!   or may come to read, as clearing it. `fetch` and `pull` have no `--repo`.
//! - The **repository word** is the first word that gives a repository so, as Git takes
//!   the first word that is no option as the repository whatever a `--repo` says, and
//!   only where there is none the value of a `--repo` that stands; a **refspec** is any
//!   later word that would give one, so that an option and a word presumed a value after
//!   the repository word are not refspecs. The reading serves the hints alone: the
//!   comparison of destinations takes every word whatever it is.
//! - A branch has an upstream for `push` only while both `branch.<name>.remote` and
//!   `branch.<name>.merge` are reported, and for `pull` while the second is (S13). Git
//!   compares the raw value of the one `merge` with `refs/heads/<name>`, so a value
//!   without that prefix names a branch of another name even where it spells the
//!   branch's own.
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

/// What the words after a transfer command give it in place of the default remote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repository<'a> {
    /// No repository: Git takes the default remote in its place.
    Default,
    /// The repository word, with no refspec after it.
    Alone(&'a [u8]),
    /// The repository word, with a refspec after it.
    WithRefspec(&'a [u8]),
}

impl<'a> Repository<'a> {
    /// The repository word given, with a refspec or without; `None` given no repository.
    pub fn word(self) -> Option<&'a [u8]> {
        match self {
            Repository::Default => None,
            Repository::Alone(word) | Repository::WithRefspec(word) => Some(word),
        }
    }
}

/// What `words`, those after the command, give `transfer` in place of the default remote.
pub fn repository<'a>(transfer: Transfer, words: &[&'a [u8]]) -> Repository<'a> {
    let known = transfer.known();
    let mut by_repo = None;
    let mut first = None;
    let mut presumes_the_next = false;
    for (at, &word) in words.iter().enumerate() {
        let presumed = presumes_the_next;
        let dashed = word.starts_with(b"-");
        presumes_the_next = dashed && !word.contains(&b'=') && !known.contains(&word);
        if transfer == Transfer::Push && clears_repo(word) {
            by_repo = None;
        }
        if presumed {
            continue;
        }
        if word == b"--" {
            return match (first, &words[at + 1..]) {
                (Some(word), &[]) | (None, &[word]) => Repository::Alone(word),
                (None, &[]) => by_repo.map_or(Repository::Default, Repository::Alone),
                (Some(word), _) | (None, &[word, ..]) => Repository::WithRefspec(word),
            };
        }
        if !dashed {
            if let Some(first) = first {
                return Repository::WithRefspec(first);
            }
            first = Some(word);
        } else if transfer == Transfer::Push {
            if let Some(value) = word.strip_prefix(b"--repo=") {
                by_repo = Some(value);
            } else if word == b"--repo"
                && let Some(&value) = words.get(at + 1)
            {
                by_repo = Some(value);
            }
        }
    }
    first
        .or(by_repo)
        .map_or(Repository::Default, Repository::Alone)
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

/// Every member of a remote group the configuration's `records` give, each with its key
/// as the configuration printed it, in the records' order: the words of the value of a
/// key whose section is `remotes`, split at spaces, tabs, and newlines as Git splits it
/// (S13). Git reads what follows `remotes.` as the group's name, a subsection's dot
/// included, so every such key is a group here. A key without a value has no member.
pub fn group_members<'a>(records: &[Record<'a>]) -> Vec<(&'a [u8], &'a [u8])> {
    records
        .iter()
        .filter(|&&(key, _)| {
            Key::of(key).is_some_and(|key| key.section.eq_ignore_ascii_case(b"remotes"))
        })
        .filter_map(|&(key, value)| Some((key, value?)))
        .flat_map(|(key, value)| {
            value
                .split(|byte| matches!(byte, b' ' | b'\t' | b'\n'))
                .filter(|member| !member.is_empty())
                .map(move |member| (key, member))
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
    let found = first_set(transfer, records, branch)?;
    let a_remote = records
        .iter()
        .any(|&(key, value)| value.is_some() && is(key, b"remote", Some(found.value), b"url"));
    (!a_remote).then_some(found)
}

/// The first of `transfer`'s default keys on `branch` that the configuration's `records`
/// report, with its last record's value, the empty value for a record without one.
fn first_set<'a>(
    transfer: Transfer,
    records: &[Record<'a>],
    branch: Option<&[u8]>,
) -> Option<DefaultRemote<'a>> {
    default_keys(transfer, branch)
        .into_iter()
        .find_map(|(section, name, variable)| last(records, section, name, variable))
}

/// The last of the configuration's `records` for the key `<section>.<name>.<variable>`,
/// whose value is the key's, the empty value for a record without one.
fn last<'a>(
    records: &[Record<'a>],
    section: &[u8],
    name: Option<&[u8]>,
    variable: &[u8],
) -> Option<DefaultRemote<'a>> {
    records
        .iter()
        .rev()
        .find(|&&(key, _)| is(key, section, name, variable))
        .map(|&(key, value)| DefaultRemote {
            key,
            value: value.unwrap_or_default(),
        })
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

/// The remote `push`, given no repository, takes on `branch`, or on a detached `HEAD` when
/// `None`, where the configuration's `records` choose one (`push_chooses_no_remote` false):
/// the value of the first of its three keys reported, a remote's name or a URL, and
/// otherwise `origin`, which then has a URL (S13).
pub fn push_remote<'a>(records: &[Record<'a>], branch: Option<&[u8]>) -> &'a [u8] {
    first_set(Transfer::Push, records, branch).map_or(b"origin", |found| found.value)
}

/// Whether `word`, the repository `transfer` is given with no refspec on `branch`, is the
/// remote Git compares it with before its advice for a branch with no upstream names a
/// plain command (S13): the value of `branch.<branch>.remote` in the configuration's
/// `records`, and, for `push` where that key is unset, the one remote the records define,
/// the word counted as one, when they then define exactly one, and `origin` otherwise,
/// whatever `branch.<branch>.pushRemote` and `remote.pushDefault` hold. Both compare
/// bytes, as Git compares a remote's name.
pub fn is_the_branch_remote(
    transfer: Transfer,
    records: &[Record<'_>],
    branch: &[u8],
    word: &[u8],
) -> bool {
    match last(records, b"branch", Some(branch), b"remote") {
        Some(found) => found.value == word,
        None => {
            transfer == Transfer::Push
                && (word == b"origin" || records.iter().all(|&(key, _)| !defines(key, word)))
        }
    }
}

/// Whether `key` defines a remote other than `word`. Git defines one for every
/// `remote.<name>.<variable>` key, whatever the variable and with a value or without,
/// except where the name begins with `/`, which it warns of and skips; an empty name
/// defines the remote `.<variable>` (`remote.c`'s `handle_config` and `make_remote`, at
/// 2.43.0 and 2.56.0).
fn defines(key: &[u8], word: &[u8]) -> bool {
    Key::of(key).is_some_and(|key| {
        key.section.eq_ignore_ascii_case(b"remote")
            && match key.name {
                None => false,
                Some(b"") => word.strip_prefix(b".") != Some(key.variable),
                Some(name) => !name.starts_with(b"/") && name != word,
            }
    })
}

/// Whether the configuration's `records` give `branch` an upstream for `transfer`: a
/// `branch.<branch>.merge` reported, with a value or without, and, for `push`, a
/// `branch.<branch>.remote` too, either of which unset `push` counts as none (S13).
pub fn has_upstream(transfer: Transfer, records: &[Record<'_>], branch: &[u8]) -> bool {
    let reported = |variable: &[u8]| {
        records
            .iter()
            .any(|&(key, _)| is(key, b"branch", Some(branch), variable))
    };
    reported(b"merge") && (transfer != Transfer::Push || reported(b"remote"))
}

/// The upstream of `branch` for `push`, from the configuration's `records`, where its name
/// is not the branch's own: the value of `branch.<branch>.remote`, and the one value of
/// `branch.<branch>.merge`, other than `refs/heads/<branch>`, as Git compares them (S13).
/// `None` where `push` counts no upstream, and where `merge` has several records, for
/// which Git refuses naming no command, or one without a value, which it cannot read.
pub fn upstream_of_another_name<'a>(records: &[Record<'a>], branch: &[u8]) -> Option<Upstream<'a>> {
    if !has_upstream(Transfer::Push, records, branch) {
        return None;
    }
    let mut merges = records
        .iter()
        .filter(|&&(key, _)| is(key, b"branch", Some(branch), b"merge"));
    let (Some(&(_, Some(merge))), None) = (merges.next(), merges.next()) else {
        return None;
    };
    let own = [&b"refs/heads/"[..], branch].concat();
    let remote = last(records, b"branch", Some(branch), b"remote")?.value;
    (merge != own).then_some(Upstream { remote, merge })
}

/// A branch's upstream as `push` reads it: the remote, and the raw value of `merge`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Upstream<'a> {
    pub remote: &'a [u8],
    pub merge: &'a [u8],
}

/// Whether the configuration's `records` leave `push.default` unset or `simple`, under
/// which `push` refuses to push a branch to an upstream of another name (S13): its last
/// record, whose value Git reads exactly; a record without a value is neither.
pub fn push_default_is_simple(records: &[Record<'_>]) -> bool {
    match records
        .iter()
        .rev()
        .find(|&&(key, _)| is(key, b"push", None, b"default"))
    {
        None => true,
        Some(&(_, value)) => value == Some(b"simple"),
    }
}

/// Where `HEAD` stands for `pull`, whose `branch.<name>.rebase` is that of the branch it
/// names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Current<'a> {
    /// On this branch.
    Branch(&'a [u8]),
    /// Detached: no branch key is read.
    Detached,
    /// Any branch: for a `pull` given a repository, whether or not `HEAD` was read for
    /// its tracking hint (`Holds/G18`), and wherever `HEAD` was not read.
    AnyBranch,
}

/// Whether the configuration's `records` tell `pull` on `current` how to reconcile
/// divergent branches: `pull.rebase`, `branch.<name>.rebase` of the branch `HEAD` names,
/// or `pull.ff` reported, with a value or without, which Git reads as true. On any
/// branch, a `branch.<name>.rebase` of any branch counts, so that the hint is withheld
/// rather than given where that key may decide.
pub fn reconciliation_configured(records: &[Record<'_>], current: Current<'_>) -> bool {
    records.iter().any(|&(key, _)| {
        let rebase_of = |branch: Option<&[u8]>| {
            Key::of(key).is_some_and(|key| {
                key.section.eq_ignore_ascii_case(b"branch")
                    && key.variable.eq_ignore_ascii_case(b"rebase")
                    && key.name.is_some()
                    && (branch.is_none() || key.name == branch)
            })
        };
        is(key, b"pull", None, b"rebase")
            || is(key, b"pull", None, b"ff")
            || match current {
                Current::Branch(branch) => rebase_of(Some(branch)),
                Current::Detached => false,
                Current::AnyBranch => rebase_of(None),
            }
    })
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
mod tests;
