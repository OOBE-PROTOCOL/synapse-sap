pub mod global;
pub mod agent;
pub mod feedback;
pub mod indexing;
pub mod vault;
pub mod tools;
pub mod escrow;
pub mod attestation;
pub mod ledger;

// Legacy modules — gated behind the "legacy-memory" feature.
// Code is preserved, excluded from default binary to reduce deploy cost.
// Enable with: anchor build -- --features legacy-memory
#[cfg(feature = "legacy-memory")]
pub mod plugin;
#[cfg(feature = "legacy-memory")]
pub mod memory;
#[cfg(feature = "legacy-memory")]
pub mod buffer;
#[cfg(feature = "legacy-memory")]
pub mod digest;

// Wildcard re-exports are required so that Anchor's #[program] macro
// can find the auto-generated __client_accounts_* modules at the crate root.
pub use global::*;
pub use agent::*;
pub use feedback::*;
pub use indexing::*;
pub use vault::*;
pub use tools::*;
pub use escrow::*;
pub use attestation::*;
pub use ledger::*;

#[cfg(feature = "legacy-memory")]
pub use plugin::*;
#[cfg(feature = "legacy-memory")]
pub use memory::*;
#[cfg(feature = "legacy-memory")]
pub use buffer::*;
#[cfg(feature = "legacy-memory")]
pub use digest::*;
