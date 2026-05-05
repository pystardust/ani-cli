//! `proxy_base_url` command — the frontend asks the backend where the
//! local stream proxy is listening so it can build `<video src>` URLs.

use crate::error::Result;

/// Returns `http://127.0.0.1:<port>` (no trailing slash).
///
/// # Errors
/// Currently never errors; returns `Result` for shape consistency with
/// other commands.
pub fn proxy_base_url(state: &crate::app::AppState) -> Result<String> {
    Ok(state.proxy_origin.base.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    fn make_state(port: u16) -> AppState {
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", port),
            ani_cli_path: PathBuf::from("/x"),
            history_path: PathBuf::from("/y/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(1)),
        }
    }

    #[test]
    fn returns_origin_base_string() {
        let s = make_state(40_000);
        assert_eq!(proxy_base_url(&s).unwrap(), "http://127.0.0.1:40000");
    }
}
