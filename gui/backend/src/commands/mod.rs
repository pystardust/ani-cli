//! Plain command bodies the HTTP API in [`crate::api`] mounts as routes.
//!
//! Each function returns `Result<T, AniError>` so the API can map errors
//! to HTTP status codes + a JSON body that carries a stable i18n key
//! (see [`crate::i18n::keys`]). No command ever returns a localized
//! string — the frontend owns user-facing copy.

pub mod aniskip;
pub mod app_info;
pub mod availability;
pub mod download;
pub mod external_player;
pub mod history;
pub mod kitsu;
pub mod kitsu_warm;
pub mod play;
pub mod play_cache;
pub mod play_resolution_cache;
pub mod play_select;
pub mod proxy_url;
pub mod session;
pub mod settings;

pub use app_info::app_info;
pub use external_player::{open_external_player, LaunchArgs};
pub use history::{history_clear, history_list};
pub use proxy_url::proxy_base_url;
pub use session::{
    create_session, create_session_with_kind, CreateSessionArgs, CreateSessionResponse,
};
