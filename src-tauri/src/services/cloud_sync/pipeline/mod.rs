//! The sync pipeline. Phase 0 ships [`buckets`] (bucket identity) and [`dirty`]
//! (per-mutation HLC stamping, dirty-queue, tombstones). Phase 1 adds JSONL
//! serialize/parse, the merge engine, and local manifest computation here.

pub mod buckets;
pub mod dirty;
