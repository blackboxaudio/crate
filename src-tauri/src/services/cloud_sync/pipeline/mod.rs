//! The sync pipeline. Phase 0 ships [`buckets`] (bucket identity) and [`dirty`]
//! (per-mutation HLC stamping, dirty-queue, tombstones). Phase 1 adds JSONL
//! serialize/parse ([`rows`]), the merge engine ([`merge`]), and local manifest
//! computation ([`manifest`]). Phase 2 adds the push pipeline ([`push`]); Phase 3 the
//! pull pipeline ([`pull`]) and the GC sweep ([`gc`]).

pub mod buckets;
pub mod dirty;
pub mod gc;
pub mod manifest;
pub mod merge;
pub mod pull;
pub mod push;
pub mod rows;
