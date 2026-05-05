//! Reader and writer for the shared `ani-hsts` history file.
//!
//! Format (TSV, one record per line):
//!     <ep_no>\t<id>\t<title>
//!
//! `ani-cli` itself reads/writes this file via its `update_history` and
//! `process_hist_entry` functions; tests in `tests/bash/` characterize that
//! contract. The GUI must produce byte-identical output (atomic
//! `path.new` + rename) so a user alternating between CLI and GUI sees a
//! single coherent history. Implementation lands in M1.6.
