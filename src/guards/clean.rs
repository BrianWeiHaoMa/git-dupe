//! `clean`'s refusals and the `-e` patterns that make Git's own `clean` spare the hidden
//! paths (G16, `Holds/G16`), decided over what the caller found: the symbolic links
//! `lstat` found among the hidden paths' ancestors, the region paths, the
//! ancestor directories `lstat` found holding a `.git` entry and, under `-X` without `-d`
//! or a pathspec, the region paths it found to be directories, the tracked paths at or
//! below either, and, given a non-literal pathspec, below the outermost ancestor of each
//! region path, in the index `clean` reads, and, under `-X`, the ancestors public Git
//! ignores. This part runs nothing and reads no file; the rule of a path is the keeper's
//! one rule form, handed in by the caller, so that nothing here escapes a path a second
//! way.
//!
//! What the code below cannot show:
//!
//! - The patterns are ignore rules of the highest precedence, read in order, the later
//!   deciding (S4). So the user's own `-e` patterns stand before these, and among these
//!   an ancestor's pair stands before every pair below it and before every spared path's.
//! - Git adds no pathspec of git-dupe's to the run: a pathspec makes `clean` delete
//!   untracked directories without `-d` (S4). Everything is said through `-e`.

use std::collections::BTreeSet;

use super::operand;

/// Whether the words are refused before any run of `clean`'s own: `-X`, whose candidates
/// are the ignored paths that git-dupe's patterns un-ignore, given together with a pattern
/// of the user's own, in any spelling (G16). Which words spell either is the front's
/// reading; a hidden path beyond a symbolic link refuses first (`refused_beyond_a_link`).
pub fn refused(only_ignored: bool, users_patterns: bool) -> bool {
    only_ignored && users_patterns
}

/// The refusal of every form while a hidden path has a symbolic link among its
/// ancestors (G16): the first of `hidden`, in their order, for which `link_above`, the
/// caller's `lstat`, finds one, with the outermost such link. Git never looks beyond the
/// link, and its `clean` can delete the link itself as one entry, without following it
/// (S5), which no pattern of a hidden path can spare: the hidden path's only way to what
/// stands at it.
pub fn refused_beyond_a_link<'p>(
    hidden: impl IntoIterator<Item = &'p [u8]>,
    mut link_above: impl FnMut(&'p [u8]) -> Option<&'p [u8]>,
) -> Option<(&'p [u8], &'p [u8])> {
    hidden
        .into_iter()
        .find_map(|path| link_above(path).map(|link| (path, link)))
}

/// Whether `word`, one of the user's pathspecs, is a non-literal pathspec (G16): one
/// holding `*`, `?`, `[`, or `\`, or beginning with `:` other than `:/`. It is read by its
/// bytes alone, as `operand::rooted_or_literal` reads an operand of `status` or `add`,
/// with `\` added, which Git reads as an escape in a pathspec it does not take literally:
/// such a word can match a directory and fail, or leave out, a file below it, which a
/// literal path cannot (S4). No magic is parsed and no setting before `dupe` is read, so
/// that under `--literal-pathspecs`, and for `top`, `literal`, or `icase` magic on a word
/// holding none of the four bytes, the answer is over-inclusive (`Holds/G16`).
pub fn non_literal(word: &[u8]) -> bool {
    operand::rooted_or_literal(word).is_err() || word.contains(&b'\\')
}

/// The outermost proper ancestor below the root of each region path that has one: what
/// the caller's listing is also asked about when a non-literal pathspec is given, so that
/// it names every path the index holds below an ancestor of a region path (`loose`).
pub fn outermost_ancestors(region: &[Vec<u8>]) -> BTreeSet<Vec<u8>> {
    region
        .iter()
        .filter_map(|path| operand::ancestors(path).next())
        .map(<[u8]>::to_vec)
        .collect()
}

/// The refusal of a non-literal pathspec over a loose directory (G16): the first region
/// path, in the order of `region`, with a loose directory above it, and the outermost
/// such directory. A loose directory is a proper ancestor below the root of a region path
/// under which `publicly_tracked`, the caller's listing under `outermost_ancestors` in the
/// index the `clean` run reads, names no path: Git's `clean`, given a pathspec, reads it
/// whole, and deletes it whole unless its walk met inside it an entry a rule excludes
/// (S4). The test is lexical: nothing need stand at the directory, because where nothing
/// or a file does, Git can delete nothing below it and the refusal costs nothing. Only
/// the region paths are asked about, because every other hidden path lies below one of
/// them, inside what its pattern spares.
pub fn loose<'r>(
    region: &'r [Vec<u8>],
    publicly_tracked: &BTreeSet<Vec<u8>>,
) -> Option<(&'r [u8], &'r [u8])> {
    region.iter().find_map(|path| {
        operand::ancestors(path)
            .find(|ancestor| !operand::any_at_or_below(publicly_tracked, ancestor))
            .map(|directory| (path.as_slice(), directory))
    })
}

/// What the user's words ask of the patterns: Git's standard rules or `-x`, where a
/// spared path is excluded, or `-X`, where it is un-ignored.
pub enum Arrangement<'a> {
    /// Without `-X`: each spared path excluded, so neither it nor anything below it is a
    /// candidate.
    Excluded,
    /// With `-X`: the ancestors public Git ignores lifted for what lies directly below
    /// them, each spared path un-ignored, and, when `keep_directories_whole`, a directory
    /// at a spared path kept out as one ignored entry, unless `publicly_tracked` holds a
    /// path at or below it.
    Unignored {
        ignored_ancestors: &'a BTreeSet<Vec<u8>>,
        /// The words hold neither `-d` nor a pathspec.
        keep_directories_whole: bool,
        /// The paths the caller's listing found in the index the `clean` run reads. Git
        /// enters a directory under which that index holds a path whatever the patterns
        /// say, and everything below an excluded directory is ignored (S4), so such a
        /// directory at a spared path is never kept out: what lies in it would be taken.
        publicly_tracked: &'a BTreeSet<Vec<u8>>,
    },
}

/// The nested repositories as G16 counts them: each directory of `holding_git`, the
/// ancestors of the region paths found holding a `.git` entry, whether or not Git would
/// open it, at or below which `publicly_tracked`, the paths under them in the index the
/// `clean` run reads, holds none. Git enters a directory under which that index tracks a
/// path like any other, whatever stands in it (S4), so the region paths in it are spared
/// one by one.
pub fn nested(
    holding_git: &BTreeSet<Vec<u8>>,
    publicly_tracked: &BTreeSet<Vec<u8>>,
) -> BTreeSet<Vec<u8>> {
    holding_git
        .iter()
        .filter(|directory| !operand::any_at_or_below(publicly_tracked, directory))
        .cloned()
        .collect()
}

/// The spared paths: each region path that no directory in `nested` lies above, and, in
/// place of the others, the outermost directory in `nested` above each, once. `nested`
/// holds the nested repositories as G16 counts them (`nested`). In the order of
/// `region`, the first place each is reached.
pub fn spared(region: &[Vec<u8>], nested: &BTreeSet<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut spared: Vec<Vec<u8>> = Vec::new();
    for path in region {
        let outermost = operand::ancestors(path).find(|ancestor| nested.contains(*ancestor));
        let path = outermost.unwrap_or(path);
        if !spared.iter().any(|kept| kept == path) {
            spared.push(path.to_vec());
        }
    }
    spared
}

/// Every proper ancestor below the root of each spared path, each once: what the `-X`
/// question asks public Git about, once the caller has left out each at which something
/// other than a directory stands, a symbolic link included, and each beyond a link.
pub fn ancestors(spared: &[Vec<u8>]) -> BTreeSet<Vec<u8>> {
    spared
        .iter()
        .flat_map(|path| operand::ancestors(path))
        .map(<[u8]>::to_vec)
        .collect()
}

/// The `-e` options for `spared`, as the words `-e` and a pattern, in order, `rule` giving
/// the region rule of a path (`Composition/Keeper`).
pub fn patterns(
    spared: &[Vec<u8>],
    arrangement: &Arrangement,
    rule: impl Fn(&[u8]) -> Vec<u8>,
) -> Vec<Vec<u8>> {
    let mut patterns = Vec::new();
    match arrangement {
        Arrangement::Excluded => {
            for path in spared {
                patterns.push(rule(path));
            }
        }
        Arrangement::Unignored {
            ignored_ancestors,
            keep_directories_whole,
            publicly_tracked,
        } => {
            // Byte order puts each ancestor before every path below it: outermost first.
            for ancestor in ignored_ancestors.iter() {
                let rule = rule(ancestor);
                patterns.push([b"!", &rule[..]].concat());
                patterns.push([&rule[..], b"/*"].concat());
            }
            for path in spared {
                let rule = rule(path);
                patterns.push([b"!", &rule[..]].concat());
                patterns.push([b"!", &rule[..], b"/**"].concat());
                if *keep_directories_whole && !operand::any_at_or_below(publicly_tracked, path) {
                    patterns.push([&rule[..], b"/"].concat());
                }
            }
        }
    }
    patterns
        .into_iter()
        .flat_map(|pattern| [b"-e".to_vec(), pattern])
        .collect()
}

#[cfg(test)]
mod tests;
