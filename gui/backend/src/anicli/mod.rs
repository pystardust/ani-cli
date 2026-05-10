//! `ani-cli` subprocess driver.
//!
//! The backend never reimplements scraping. It always shells out to the
//! vendored `ani-cli` script, parsing its `ANI_CLI_PLAYER=debug` stdout to
//! recover the resolved stream URL, referer, and subtitle URL.
//!
//! Submodules:
//!
//! - `parser` — ANSI-strip and grammar for the debug output. Pure, easy to
//!   property-test.
//! - `process` — `tokio::process::Command` with `kill_on_drop`,
//!   `TERM=dumb`, `NO_COLOR=1`, and a wall-clock timeout. The function
//!   that actually spawns the script.

pub mod bash;
pub mod parser;
pub mod process;
pub mod update;

pub use parser::{DebugOutput, SearchResult};
pub use process::{run_debug, run_search};
