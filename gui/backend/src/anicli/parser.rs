//! Stdout parser for `ani-cli` invocations.
//!
//! Two output shapes the backend cares about:
//!
//! 1. Search results — emitted before the episode prompt:
//!    `<id>\t<title> (<n> episodes)`
//! 2. Debug-mode resolved stream — emitted by `ANI_CLI_PLAYER=debug`:
//!    ```text
//!    All links:
//!    <quality> >https://...
//!    <quality>cc>https://...
//!    subtitle >https://...
//!    m3u8_refr >https://...
//!    Selected link:
//!    https://...
//!    ```
//!
//! The functions here strip ANSI escapes, then run regex/split-based
//! extraction. They are pure (no I/O) and deterministic — ideal for
//! property tests.

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};

/// One row returned by `ani-cli`'s search step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    /// Allanime show id (alphanumeric).
    pub id: String,
    /// Display title.
    pub title: String,
    /// Episode count for the active mode (sub or dub).
    pub episode_count: u32,
}

/// One line of progress emitted on `ani-cli`'s stderr while it
/// resolves a stream. Used to forward incremental status to the
/// renderer's loading overlay over SSE so the user sees something
/// happening during the 8-30 s wait.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProgressLine {
    /// A startup banner (`Checking dependencies...`).
    Banner {
        /// The full banner text, ANSI-stripped.
        text: String,
    },
    /// `<provider> Links Fetched` — emitted by `provider_init` for
    /// each embed source ani-cli successfully queried. Drives the
    /// "youtube → sharepoint → wixmp → hianime" trail in the UI.
    LinksFetched {
        /// Provider label as printed by ani-cli (`youtube`,
        /// `sharepoint`, `wixmp`, `hianime`, …).
        provider: String,
    },
    /// Any other line we don't recognise; passed through verbatim
    /// so the overlay can fall back to "raw output" mode if upstream
    /// adds a new line we haven't taught the parser about.
    Other {
        /// The original line, ANSI-stripped and trimmed.
        text: String,
    },
}

/// Classify a single (already ANSI-stripped) line of `ani-cli` stderr
/// into a [`ProgressLine`].
///
/// Returns `None` for empty or whitespace-only lines so the SSE stream
/// doesn't ferry blank events to the renderer.
///
/// # Drift contract
///
/// The `<provider> Links Fetched` shape comes from `ani-cli`'s
/// `provider_init` (`printf "\033[1;32m%s\033[0m Links Fetched\n"`).
/// If upstream changes that format, this parser falls back to
/// `ProgressLine::Other` and the overlay stops showing the friendly
/// label — but playback still works. The integration drift test in
/// `tests/anicli_progress_format.rs` runs real `ani-cli` through the
/// curl shim and fails loudly if the expected format disappears.
#[must_use]
pub fn parse_progress_line(line: &str) -> Option<ProgressLine> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("Checking dependencies") {
        return Some(ProgressLine::Banner {
            text: trimmed.to_string(),
        });
    }
    if let Some(provider) = trimmed.strip_suffix(" Links Fetched") {
        let provider = provider.trim();
        if !provider.is_empty() {
            return Some(ProgressLine::LinksFetched {
                provider: provider.to_string(),
            });
        }
    }
    Some(ProgressLine::Other {
        text: trimmed.to_string(),
    })
}

/// Parsed output of `ANI_CLI_PLAYER=debug ani-cli ...`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugOutput {
    /// The link `select_quality` chose.
    pub selected_url: String,
    /// All candidate links, in the order ani-cli emitted them.
    pub all_links: Vec<String>,
    /// `Referer:` value to send with stream requests, if any.
    pub referer: Option<String>,
    /// Subtitle .vtt URL, if any.
    pub subtitle_url: Option<String>,
}

/// Strip ANSI escape sequences from a byte slice and decode lossy UTF-8.
#[must_use]
pub fn strip_ansi(bytes: &[u8]) -> String {
    let cleaned = strip_ansi_escapes::strip(bytes);
    String::from_utf8_lossy(&cleaned).into_owned()
}

/// Parse search-results lines into `SearchResult`s. The expected line
/// format is `id<TAB>title (N episodes)`. Lines that don't match the
/// pattern are silently skipped — `ani-cli` mixes log lines with results.
#[must_use]
pub fn parse_search_results(stdout: &str) -> Vec<SearchResult> {
    stdout.lines().filter_map(parse_search_line).collect()
}

fn parse_search_line(line: &str) -> Option<SearchResult> {
    // Format: `id\ttitle (N episodes)`
    let (id, rest) = line.split_once('\t')?;
    let id = id.trim();
    if id.is_empty() {
        return None;
    }

    // The title may itself contain parentheses, so we rsplit on `(` to find
    // the last "(N episodes)" group.
    let (title_with_space, count_part) = rest.rsplit_once('(')?;
    let title = title_with_space.trim_end().to_string();
    if title.is_empty() {
        return None;
    }
    let count_part = count_part.trim();
    let count_str = count_part
        .strip_suffix(" episodes)")
        .or_else(|| count_part.strip_suffix(" episode)"))?;
    let episode_count = count_str.parse::<u32>().ok()?;
    Some(SearchResult {
        id: id.to_string(),
        title,
        episode_count,
    })
}

/// Parse `ANI_CLI_PLAYER=debug` output.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] if the stdout doesn't include the
/// `Selected link:` marker the debug branch is supposed to print.
pub fn parse_debug_output(stdout: &str) -> Result<DebugOutput> {
    let stdout = stdout.trim();

    // Find the "Selected link:" marker. Everything before it (after the
    // "All links:" header) is the link list; the line after the marker is
    // the chosen URL.
    let selected_idx = stdout
        .find("Selected link:")
        .ok_or_else(|| AniError::ParseFailed {
            detail: "no 'Selected link:' marker".into(),
        })?;

    let (links_part, after_selected) = stdout.split_at(selected_idx);
    let after_selected = after_selected
        .trim_start_matches("Selected link:")
        .trim_start();
    let selected_url = after_selected
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AniError::ParseFailed {
            detail: "no URL after 'Selected link:'".into(),
        })?
        .to_string();

    // Strip the optional "All links:" header and a trailing newline.
    let trimmed = links_part.trim();
    let links_block = trimmed
        .strip_prefix("All links:")
        .map_or(trimmed, str::trim_start);

    // Pull subtitle and m3u8_refr metadata lines out of the link list.
    let mut all_links = Vec::new();
    let mut subtitle_url = None;
    let mut referer = None;
    for raw in links_block.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("subtitle >") {
            subtitle_url = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("m3u8_refr >") {
            referer = Some(rest.trim().to_string());
            continue;
        }
        all_links.push(line.to_string());
    }

    Ok(DebugOutput {
        selected_url,
        all_links,
        referer,
        subtitle_url,
    })
}

#[cfg(test)]
#[allow(missing_docs)]
mod progress_tests {
    use super::*;

    #[test]
    fn parse_progress_line_classifies_links_fetched_with_provider_name() {
        assert_eq!(
            parse_progress_line("youtube Links Fetched"),
            Some(ProgressLine::LinksFetched {
                provider: "youtube".into()
            })
        );
        assert_eq!(
            parse_progress_line("sharepoint Links Fetched"),
            Some(ProgressLine::LinksFetched {
                provider: "sharepoint".into()
            })
        );
    }

    #[test]
    fn parse_progress_line_classifies_dependency_banner() {
        assert_eq!(
            parse_progress_line("Checking dependencies..."),
            Some(ProgressLine::Banner {
                text: "Checking dependencies...".into()
            })
        );
    }

    #[test]
    fn parse_progress_line_passes_unknown_lines_through_as_other() {
        assert_eq!(
            parse_progress_line("Some unrecognised log line"),
            Some(ProgressLine::Other {
                text: "Some unrecognised log line".into()
            })
        );
    }

    #[test]
    fn parse_progress_line_strips_whitespace_around_provider_name() {
        // The strip_ansi step usually leaves trailing spaces from the
        // colour reset bytes; the parser should tolerate them.
        assert_eq!(
            parse_progress_line("  hianime   Links Fetched"),
            Some(ProgressLine::LinksFetched {
                provider: "hianime".into()
            })
        );
    }

    #[test]
    fn parse_progress_line_returns_none_for_blank_lines() {
        assert_eq!(parse_progress_line(""), None);
        assert_eq!(parse_progress_line("   \t  "), None);
    }

    #[test]
    fn parse_progress_line_handles_real_post_strip_input() {
        // Output captured from `ani-cli -S 1 -e 1 -q best "Test"`,
        // stderr only, after running through strip_ansi. Pinning a
        // representative snippet so a regression in strip_ansi or
        // the parser is loud.
        let lines = ["Checking dependencies...", "youtube Links Fetched", ""];
        let parsed: Vec<_> = lines
            .iter()
            .filter_map(|l| parse_progress_line(l))
            .collect();
        assert_eq!(
            parsed,
            vec![
                ProgressLine::Banner {
                    text: "Checking dependencies...".into()
                },
                ProgressLine::LinksFetched {
                    provider: "youtube".into()
                },
            ]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_removes_escape_codes() {
        let raw = b"\x1b[1;31mred\x1b[0m text";
        let out = strip_ansi(raw);
        assert_eq!(out, "red text");
    }

    #[test]
    fn parse_search_one_line() {
        let line = "abc123\tOne Piece (1100 episodes)";
        let parsed = parse_search_results(line);
        assert_eq!(parsed.len(), 1);
        let r = &parsed[0];
        assert_eq!(r.id, "abc123");
        assert_eq!(r.title, "One Piece");
        assert_eq!(r.episode_count, 1100);
    }

    #[test]
    fn parse_search_handles_parens_in_title() {
        // The title contains its own parentheses; only the last `(N
        // episodes)` group is the count.
        let line = "xyz\tFullmetal Alchemist (Brotherhood) (64 episodes)";
        let parsed = parse_search_results(line);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].title, "Fullmetal Alchemist (Brotherhood)");
        assert_eq!(parsed[0].episode_count, 64);
    }

    #[test]
    fn parse_search_skips_non_matching_lines() {
        let stdout = "Checking dependencies...\n\
                      abc\tFoo (12 episodes)\n\
                      garbage line without tab\n\
                      def\tBar (1 episode)\n";
        let parsed = parse_search_results(stdout);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "abc");
        assert_eq!(parsed[1].id, "def");
        // Singular "1 episode" is accepted too.
        assert_eq!(parsed[1].episode_count, 1);
    }

    #[test]
    fn parse_debug_minimal() {
        let stdout = "All links:\n\
                      720 >https://example.com/720.mp4\n\
                      Selected link:\n\
                      https://example.com/720.mp4\n";
        let d = parse_debug_output(stdout).unwrap();
        assert_eq!(d.selected_url, "https://example.com/720.mp4");
        assert_eq!(
            d.all_links,
            vec!["720 >https://example.com/720.mp4".to_string()]
        );
        assert_eq!(d.referer, None);
        assert_eq!(d.subtitle_url, None);
    }

    #[test]
    fn parse_debug_with_m3u8_subs_and_refr() {
        let stdout = "All links:\n\
                      1080cc>https://example.com/1080.m3u8\n\
                      720cc>https://example.com/720.m3u8\n\
                      subtitle >https://example.com/sub.vtt\n\
                      m3u8_refr >https://allmanga.to\n\
                      Selected link:\n\
                      https://example.com/1080.m3u8\n";
        let d = parse_debug_output(stdout).unwrap();
        assert_eq!(d.selected_url, "https://example.com/1080.m3u8");
        assert_eq!(
            d.subtitle_url.as_deref(),
            Some("https://example.com/sub.vtt")
        );
        assert_eq!(d.referer.as_deref(), Some("https://allmanga.to"));
        // subtitle and m3u8_refr lines are stripped from all_links.
        assert!(d.all_links.iter().all(|l| !l.starts_with("subtitle >")));
        assert!(d.all_links.iter().all(|l| !l.starts_with("m3u8_refr >")));
        assert_eq!(d.all_links.len(), 2);
    }

    #[test]
    fn parse_debug_missing_marker_errors() {
        let stdout = "Some output but no Selected marker\n";
        let err = parse_debug_output(stdout).unwrap_err();
        match err {
            AniError::ParseFailed { detail } => {
                assert!(
                    detail.contains("Selected link"),
                    "detail mentions marker: {detail}"
                );
            }
            other => panic!("expected ParseFailed, got {other:?}"),
        }
    }
}
