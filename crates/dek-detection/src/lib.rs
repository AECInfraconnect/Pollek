//! # dek-detection
//!
//! Pollek's detection-as-code engine. It loads YAML rule packs, validates their
//! framework-coverage gate, verifies integrity, and evaluates them against
//! normalized agent-activity events.
//!
//! Pipeline: `loader` (parse + validate + verify) -> `eval` (match) ->
//! `coverage` (prove framework coverage).

pub mod coverage;
pub mod eval;
pub mod loader;
pub mod spec;

pub use coverage::{build_coverage, coverage_to_yaml, Coverage};
pub use eval::{evaluate, glob_match, step_matches, Detection, ObservedEvent};
pub use loader::{
    load_pack_dir, sha256_text_lf, validate_rule, verify_and_load_pack, LoadError, PackManifest,
};
pub use spec::{RuleSpec, Severity};
