//! The sync pipeline. Phase 0 ships [`buckets`] (bucket identity) and [`dirty`]
//! (per-mutation HLC stamping, dirty-queue, tombstones). Phase 1 adds JSONL
//! serialize/parse ([`rows`]), the merge engine ([`merge`]), and local manifest
//! computation ([`manifest`]).

pub mod buckets;
pub mod dirty;
pub mod manifest;
pub mod merge;
pub mod rows;
