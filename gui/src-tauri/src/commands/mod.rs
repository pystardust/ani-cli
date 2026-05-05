//! Tauri IPC commands the frontend invokes via `invoke('cmd_name', …)`.
//!
//! Every command returns `Result<T, AniError>` so the frontend always
//! sees either a structured value or a stable i18n key (see
//! [`crate::i18n::keys`]). No command ever returns a localized string.
//!
//! The `tauri::command` attribute lives only in this module; submodules
//! return plain functions so they can be unit-tested without pulling in
//! the full Tauri runtime.

pub mod app_info;
pub mod external_player;
pub mod history;
pub mod ipc;
pub mod proxy_url;

pub use app_info::app_info;
pub use external_player::{open_external_player, LaunchArgs};
pub use history::{history_clear, history_list};
pub use ipc::{
    cmd_app_info, cmd_history_clear, cmd_history_list, cmd_open_external_player, cmd_proxy_base_url,
};
pub use proxy_url::proxy_base_url;
