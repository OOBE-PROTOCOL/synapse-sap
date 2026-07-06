pub mod agent;
pub mod attestation;
pub mod feedback;
pub mod global;
pub mod indexing;
pub mod ledger;
pub mod tools;
pub mod vault;

// ── V2.1 Modules ────────────────────────────────────────────────
pub mod dispute;
pub mod escrow_v2;
pub mod index_page;
pub mod receipt;
pub mod shards;
pub mod staking;
pub mod subscription;

// Legacy modules — gated behind the "legacy-memory" feature.
// Code is preserved, excluded from default binary to reduce deploy cost.
// Enable with: anchor build -- --features legacy-memory
#[cfg(feature = "legacy-memory")]
pub mod buffer;
#[cfg(feature = "legacy-memory")]
pub mod digest;
#[cfg(feature = "legacy-memory")]
pub mod memory;
#[cfg(feature = "legacy-memory")]
pub mod plugin;

// Wildcard re-exports are required so that Anchor's #[program] macro
// can find the auto-generated __client_accounts_* modules at the crate root.
pub use agent::*;
pub use attestation::*;
pub use feedback::*;
pub use global::*;
pub use indexing::*;
pub use ledger::*;
pub use tools::*;
pub use vault::*;

// ── V2.1 Re-exports ────────────────────────────────────────────
pub use dispute::*;
pub use escrow_v2::*;
pub use index_page::*;
pub use receipt::*;
pub use shards::*;
pub use staking::*;
pub use subscription::*;

#[cfg(feature = "legacy-memory")]
pub use buffer::*;
#[cfg(feature = "legacy-memory")]
pub use digest::*;
#[cfg(feature = "legacy-memory")]
pub use memory::*;
#[cfg(feature = "legacy-memory")]
pub use plugin::*;
