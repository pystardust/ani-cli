//! History commands — `history_list` and `history_clear`.
//!
//! Reads/writes the same `ani-hsts` file the CLI uses, so a user
//! alternating between CLI and GUI sees one coherent history.

use crate::error::Result;
use crate::history::{read_all, write_atomic, HistoryEntry};

/// Returns every history entry as the frontend would render the
/// "Continue Watching" row. Most-recent-first order is the GUI's choice;
/// the on-disk order from the CLI is "append-only with in-place updates",
/// so we return entries in the order they appear on disk and let the
/// frontend reverse if it wants newest-first.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] if the file exists but cannot
/// be read.
pub fn history_list(state: &crate::app::AppState) -> Result<Vec<HistoryEntry>> {
    read_all(&state.history_path)
}

/// Truncate the history file to zero length. Mirrors `ani-cli -D`.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] if the file cannot be written.
pub fn history_clear(state: &crate::app::AppState) -> Result<()> {
    write_atomic(&state.history_path, &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    fn make_state(history_path: PathBuf) -> AppState {
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
            ani_cli_path: PathBuf::from("/x/ani-cli"),
            history_path,
            scraper_slots: Arc::new(Semaphore::new(1)),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
            config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        }
    }

    #[test]
    fn list_empty_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_state(tmp.path().join("nope"));
        let v = history_list(&s).unwrap();
        assert!(v.is_empty());
    }

    #[test]
    fn by_kitsu_returns_the_matching_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());

        write_atomic(
            &path,
            &[
                HistoryEntry {
                    ep_no: "5".into(),
                    id: "amA".into(),
                    title: "Show A (10 episodes)".into(),
                },
                HistoryEntry {
                    ep_no: "12".into(),
                    id: "amB".into(),
                    title: "Show B (24 episodes)".into(),
                },
            ],
        )
        .unwrap();

        // Prime the (allmanga show_id → kitsu_id) reverse mapping
        // the play path stamps after a successful play.
        crate::commands::kitsu::allmanga_kitsu_put(&s, "amA", "K1").unwrap();
        crate::commands::kitsu::allmanga_kitsu_put(&s, "amB", "K2").unwrap();

        let hit = history_by_kitsu(&s, "K2").unwrap().expect("match");
        assert_eq!(hit.id, "amB");
        assert_eq!(hit.ep_no, "12");
    }

    #[test]
    fn by_kitsu_returns_none_when_no_history_entry_maps_to_id() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());

        write_atomic(
            &path,
            &[HistoryEntry {
                ep_no: "5".into(),
                id: "amA".into(),
                title: "Show A (10 episodes)".into(),
            }],
        )
        .unwrap();
        crate::commands::kitsu::allmanga_kitsu_put(&s, "amA", "K1").unwrap();

        // No history entry maps to K-other.
        assert!(history_by_kitsu(&s, "K-other").unwrap().is_none());
    }

    #[test]
    fn by_kitsu_returns_none_when_history_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_state(tmp.path().join("nope"));
        assert!(history_by_kitsu(&s, "K1").unwrap().is_none());
    }

    #[test]
    fn list_then_clear_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());
        // Pre-populate with a known fixture.
        write_atomic(
            &path,
            &[HistoryEntry {
                ep_no: "5".into(),
                id: "abc".into(),
                title: "T (10 episodes)".into(),
            }],
        )
        .unwrap();

        let listed = history_list(&s).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "abc");

        history_clear(&s).unwrap();
        let after = history_list(&s).unwrap();
        assert!(after.is_empty());
    }
}
