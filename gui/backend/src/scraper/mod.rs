//! Narrow allanime client used **only for search disambiguation**.
//!
//! ## Why this module exists
//!
//! AGENTS.md / `.planning/architecture.md` say the backend never
//! reimplements scraping — `ani-cli` is the source of truth for the
//! search → pick → embed pipeline. This module is a deliberate,
//! scoped deviation:
//!
//! - ani-cli's `-S 1` blindly picks allanime's first match. allanime's
//!   ranking puts the 1-episode side story `Naruto: Shippuuden:
//!   Shippuu! Konoha Gakuen Den` ahead of the 500-episode main show
//!   for the query `Naruto: Shippuuden`. So the user clicked the
//!   right Kitsu entry in the UI and got a wholly different show
//!   played.
//!
//! - ani-cli has no flag to expose its candidate list without going
//!   through fzf/dmenu, and we can't modify the vendored script.
//!
//! - The fix is to call the **same search GraphQL ani-cli would
//!   call**, get the candidate list, pick the entry whose
//!   `availableEpisodes` count is closest to Kitsu's, and pass the
//!   1-based index back to ani-cli via `-S <n>`. ani-cli still owns
//!   the embed pipeline (which is the load-bearing scraping work);
//!   we only decide *which of its candidates* to ask for.
//!
//! - Search request shape stays in lockstep with `ani-cli`'s own
//!   `search_anime` body. The endpoint, headers, and GraphQL query
//!   below are byte-for-byte mirrors of the script — kept that way
//!   so any drift on allanime's side breaks ani-cli first and we
//!   patch the same way upstream does.

pub mod allanime;

pub use allanime::{pick_by_ep_count, search, Candidate};
