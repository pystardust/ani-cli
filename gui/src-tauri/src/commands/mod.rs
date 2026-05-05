//! Tauri IPC commands exposed to the frontend.
//!
//! Each command is a typed wrapper that returns `Result<T, AniError>` so
//! the frontend gets either a structured value or a stable i18n key. No
//! command ever returns a localized string.
//!
//! Implementation begins in M1.4. The list below is the planned surface;
//! see `docs/architecture.md` §commands for shapes.

// Stubs come online as commands are wired up.
