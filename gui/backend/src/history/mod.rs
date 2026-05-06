//! Reader and writer for the shared `ani-hsts` history file.
//!
//! Format (TSV, one record per line):
//!     <ep_no>\t<id>\t<title>
//!
//! `ani-cli`'s `update_history` function (in the script) reads/writes this
//! file with atomic semantics: write to `path.new`, then rename. Tests in
//! `tests/bash/network/update_history.bats` characterize that contract.
//! The Rust reader/writer here must produce byte-identical output so a
//! user alternating between CLI and GUI sees a single coherent history.
//!
//! Path resolution lives in [`crate::config::paths::ani_cli_history`].

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// One row of the history file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Episode number the user last watched. `ani-cli` writes this back
    /// after each play, so on next launch the GUI's "Continue Watching"
    /// row knows where to resume.
    pub ep_no: String,
    /// Allanime show id.
    pub id: String,
    /// Display title (typically `"<name> (<n> episodes)"`).
    pub title: String,
}

/// Parse the entire history file into a `Vec<HistoryEntry>`.
///
/// A missing file returns `Ok(vec![])`. Malformed lines are silently
/// dropped — `ani-cli`'s shell parser does the same when the column count
/// doesn't match.
///
/// # Errors
/// Returns [`AniError::Io`] when the file exists but cannot be read.
pub fn read_all(path: &Path) -> Result<Vec<HistoryEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let body = std::fs::read_to_string(path)?;
    Ok(parse(&body))
}

/// Parse a TSV body into entries. Pure function, no I/O.
#[must_use]
pub fn parse(body: &str) -> Vec<HistoryEntry> {
    body.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let ep_no = parts.next()?.to_string();
            let id = parts.next()?.to_string();
            let title = parts.next()?.to_string();
            if ep_no.is_empty() || id.is_empty() {
                return None;
            }
            Some(HistoryEntry { ep_no, id, title })
        })
        .collect()
}

/// Serialize entries back to the TSV body. Each line ends with `\n`,
/// including the last one (matches what `ani-cli` writes via its
/// `printf "%s\t%s\t%s\n"` line in `update_history`).
#[must_use]
pub fn serialize(entries: &[HistoryEntry]) -> String {
    let mut out = String::with_capacity(entries.len() * 64);
    for e in entries {
        out.push_str(&e.ep_no);
        out.push('\t');
        out.push_str(&e.id);
        out.push('\t');
        out.push_str(&e.title);
        out.push('\n');
    }
    out
}

/// Insert or update an entry, matching by `id`. If `id` is already in the
/// vector, that entry's `ep_no` and `title` are replaced; otherwise the
/// new entry is appended. The vector is mutated in place. Mirrors
/// `update_history`'s semantics from `ani-cli`.
pub fn upsert(entries: &mut Vec<HistoryEntry>, new: HistoryEntry) {
    if let Some(existing) = entries.iter_mut().find(|e| e.id == new.id) {
        existing.ep_no = new.ep_no;
        existing.title = new.title;
    } else {
        entries.push(new);
    }
}

/// Remove an entry by id. Returns true if a row was removed.
pub fn remove_by_id(entries: &mut Vec<HistoryEntry>, id: &str) -> bool {
    let before = entries.len();
    entries.retain(|e| e.id != id);
    entries.len() != before
}

/// Atomically write the entire history file. Implemented as `path.new` +
/// rename, exactly as `ani-cli`'s `update_history` does. The `.new`
/// sidecar is unlinked before this function returns successfully (the
/// final `rename` overwrites the original atomically on Unix).
///
/// # Errors
/// Returns [`AniError::Io`] for I/O failures (including write or rename).
pub fn write_atomic(path: &Path, entries: &[HistoryEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let new_path = path.with_extension("new");
    let body = serialize(entries);
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&new_path)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&new_path, path)?;
    // Belt-and-suspenders: if the rename succeeded the new path is gone,
    // but if a previous run crashed mid-rename a stale `.new` could be
    // hanging around. Best-effort cleanup of any sidecar with a different
    // suffix.
    if new_path.exists() {
        let _ = std::fs::remove_file(&new_path);
    }
    Ok(())
}

/// Convenience: read + upsert + write_atomic in one call. Pure error
/// propagation; the on-disk file is mutated in-place.
///
/// # Errors
/// Returns [`AniError::Io`] on read or write failure.
pub fn upsert_and_write(path: &Path, new: HistoryEntry) -> Result<()> {
    let mut entries = read_all(path)?;
    upsert(&mut entries, new);
    write_atomic(path, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        // Repo root → tests/fixtures/history/.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("two levels up from manifest")
            .join("tests/fixtures/history")
    }

    fn sample_entry(id: &str, ep: &str) -> HistoryEntry {
        HistoryEntry {
            ep_no: ep.into(),
            id: id.into(),
            title: format!("Test ({id})"),
        }
    }

    #[test]
    fn parse_empty_yields_empty() {
        assert!(parse("").is_empty());
        assert!(parse("\n").is_empty());
    }

    #[test]
    fn parse_three_columns() {
        let body = "5\tabc\tOne Piece (1100 episodes)\n";
        let v = parse(body);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].ep_no, "5");
        assert_eq!(v[0].id, "abc");
        assert_eq!(v[0].title, "One Piece (1100 episodes)");
    }

    #[test]
    fn parse_skips_malformed_lines() {
        // First line missing tabs; second valid; third missing id.
        let body = "no-tabs-line\n\
                    1\tdef\tValid\n\
                    \t\tno-id\n";
        let v = parse(body);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "def");
    }

    #[test]
    fn parse_preserves_tabs_after_third_column() {
        // Title column may itself contain tabs (rare but possible). Only
        // the first two tabs are field separators.
        let body = "1\tid\ttitle\twith\tmore\ttabs\n";
        let v = parse(body);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].title, "title\twith\tmore\ttabs");
    }

    #[test]
    fn serialize_round_trips_through_parse() {
        let entries = vec![
            sample_entry("a", "1"),
            sample_entry("b", "5"),
            sample_entry("c", "12"),
        ];
        let body = serialize(&entries);
        let parsed = parse(&body);
        assert_eq!(parsed, entries);
    }

    #[test]
    fn serialize_ends_every_line_with_newline() {
        let entries = vec![sample_entry("a", "1")];
        let body = serialize(&entries);
        assert!(body.ends_with('\n'));
    }

    #[test]
    fn upsert_appends_when_id_is_new() {
        let mut v = vec![sample_entry("a", "1")];
        upsert(&mut v, sample_entry("b", "2"));
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].id, "b");
    }

    #[test]
    fn upsert_replaces_when_id_exists_and_keeps_position() {
        let mut v = vec![
            sample_entry("a", "1"),
            sample_entry("b", "2"),
            sample_entry("c", "3"),
        ];
        let updated = HistoryEntry {
            ep_no: "99".into(),
            id: "b".into(),
            title: "New Title".into(),
        };
        upsert(&mut v, updated);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].id, "b");
        assert_eq!(v[1].ep_no, "99");
        assert_eq!(v[1].title, "New Title");
    }

    #[test]
    fn remove_by_id_drops_the_matching_row() {
        let mut v = vec![sample_entry("a", "1"), sample_entry("b", "2")];
        assert!(remove_by_id(&mut v, "a"));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "b");
    }

    #[test]
    fn remove_by_id_is_noop_when_id_missing() {
        let mut v = vec![sample_entry("a", "1")];
        assert!(!remove_by_id(&mut v, "missing"));
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn read_all_missing_file_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("does-not-exist");
        let v = read_all(&path).unwrap();
        assert!(v.is_empty());
    }

    #[test]
    fn read_all_matches_bash_fixture_multi() {
        let v = read_all(&fixtures_dir().join("multi.tsv")).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].ep_no, "12");
        assert_eq!(v[0].id, "abc123");
        assert_eq!(v[0].title, "Attack on Titan (25 episodes)");
        assert_eq!(v[1].id, "def456");
        assert_eq!(v[2].id, "ghi789");
    }

    #[test]
    fn serialize_byte_identical_to_bash_fixture() {
        // Cross-stack contract: ani-cli's update_history writes
        //     printf "%s\t%s\t%s\n" "$ep_no" "$id" "$title"
        // Our serialize() must produce byte-identical output for the same
        // logical entries so a user alternating between CLI and GUI sees
        // one coherent history file.
        let entries = vec![
            HistoryEntry {
                ep_no: "12".into(),
                id: "abc123".into(),
                title: "Attack on Titan (25 episodes)".into(),
            },
            HistoryEntry {
                ep_no: "3".into(),
                id: "def456".into(),
                title: "Demon Slayer (26 episodes)".into(),
            },
            HistoryEntry {
                ep_no: "1".into(),
                id: "ghi789".into(),
                title: "Spy x Family (12 episodes)".into(),
            },
        ];
        let our_bytes = serialize(&entries);
        let bash_bytes = std::fs::read_to_string(fixtures_dir().join("multi.tsv")).unwrap();
        assert_eq!(our_bytes, bash_bytes, "byte-identical with bash output");
    }

    #[test]
    fn write_atomic_round_trips_disk_to_memory() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let entries = vec![sample_entry("a", "1"), sample_entry("b", "2")];
        write_atomic(&path, &entries).unwrap();
        let back = read_all(&path).unwrap();
        assert_eq!(back, entries);
        // No .new sidecar lingers.
        assert!(!path.with_extension("new").exists());
    }

    #[test]
    fn write_atomic_creates_parent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested/dir/ani-hsts");
        let entries = vec![sample_entry("a", "1")];
        write_atomic(&path, &entries).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn upsert_and_write_creates_then_updates() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");

        upsert_and_write(&path, sample_entry("a", "1")).unwrap();
        upsert_and_write(&path, sample_entry("b", "2")).unwrap();
        let v1 = read_all(&path).unwrap();
        assert_eq!(v1.len(), 2);

        // Re-upsert "a" with a new ep_no.
        upsert_and_write(
            &path,
            HistoryEntry {
                ep_no: "99".into(),
                id: "a".into(),
                title: "Test (a)".into(),
            },
        )
        .unwrap();
        let v2 = read_all(&path).unwrap();
        assert_eq!(v2.len(), 2);
        assert_eq!(v2[0].id, "a");
        assert_eq!(v2[0].ep_no, "99");
    }

    // — Properties ────────────────────────────────────────────────────
    //
    // The TSV format is shared with the bash `ani-cli` script — both
    // have to agree on what the file means. Roundtripping (serialize
    // then parse) is the load-bearing invariant: if it ever breaks,
    // history written by the GUI silently disappears the next time
    // the CLI loads the file (or vice versa).
    use proptest::prelude::*;

    /// Generate a single field that's safe to put in a TSV row.
    ///
    /// Excludes `\t` and `\n` because they're the column/row
    /// separators, and `\r` because Rust's `str::lines()` (which
    /// `parse` uses) strips a trailing `\r` from each line as part
    /// of CRLF normalization — so a title containing `\r` wouldn't
    /// roundtrip. The bash CLI on Linux writes pure `\n`, so this
    /// set of constraints matches the format we share with it.
    fn tsv_field(min_len: usize, max_len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(any::<char>(), min_len..=max_len)
            .prop_filter("no tabs, newlines, or carriage returns", |chars| {
                chars.iter().all(|c| *c != '\t' && *c != '\n' && *c != '\r')
            })
            .prop_map(|chars| chars.into_iter().collect())
    }

    fn entry_strategy() -> impl Strategy<Value = HistoryEntry> {
        // ep_no and id must be non-empty (parse() drops rows otherwise).
        // title may be empty.
        (tsv_field(1, 8), tsv_field(1, 32), tsv_field(0, 64))
            .prop_map(|(ep_no, id, title)| HistoryEntry { ep_no, id, title })
    }

    proptest! {
        /// `parse(serialize(entries)) == entries` for any well-formed
        /// vector. The format has no escaping, so the property only
        /// holds when fields are TSV-clean (no embedded tabs/newlines)
        /// — exactly what the bash CLI produces.
        #[test]
        fn parse_serialize_roundtrip(
            entries in proptest::collection::vec(entry_strategy(), 0..16),
        ) {
            let body = serialize(&entries);
            let parsed = parse(&body);
            prop_assert_eq!(entries, parsed);
        }

        /// `upsert` is idempotent on the same entry: applying it twice
        /// produces the same vector as applying it once. The CLI relies
        /// on this — replaying the same play action mustn't multiply
        /// rows.
        #[test]
        fn upsert_is_idempotent(
            initial in proptest::collection::vec(entry_strategy(), 0..8),
            new in entry_strategy(),
        ) {
            let mut once = initial.clone();
            upsert(&mut once, new.clone());
            let mut twice = once.clone();
            upsert(&mut twice, new);
            prop_assert_eq!(once, twice);
        }

        /// `remove_by_id` followed by `upsert` of the same id leaves
        /// the entry present exactly once, with the new ep_no/title.
        #[test]
        fn remove_then_upsert_yields_single_entry(
            initial in proptest::collection::vec(entry_strategy(), 0..8),
            target in entry_strategy(),
        ) {
            let mut entries = initial;
            remove_by_id(&mut entries, &target.id);
            upsert(&mut entries, target.clone());
            let matches: Vec<&HistoryEntry> = entries.iter().filter(|e| e.id == target.id).collect();
            prop_assert_eq!(matches.len(), 1);
            prop_assert_eq!(matches[0], &target);
        }
    }
}
