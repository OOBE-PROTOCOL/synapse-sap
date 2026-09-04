//! Canonical PDA seed literals for every SAP account type.
//!
//! These byte strings are **frozen protocol surfaces**: each one is baked into
//! mainnet account addresses at creation time (`seeds = [...]` in every
//! `#[derive(Accounts)]` struct and in every CPI signer seed). Renaming a
//! literal would derive different PDAs — that is a BREAKING change and is
//! forbidden without a migration.
//!
//! Centralizing the literals here (vs 65 scattered `b"sap_*"` literals across
//! 19 instruction modules) removes the single largest silent-drift vector:
//! one typo in one file = a fresh PDA family and orphaned user funds.
//!
//! Wire-compat guarantee: `mod tests` asserts every constant byte-identical
//! to the mainnet v1.0.3 literal. If that test fails, someone edited a seed
//! — stop and revert.

/// Global registry singleton. Seeds input: `[]`.
pub const GLOBAL: &[u8] = b"sap_global";

/// Agent identity — ONE PER WALLET. Seeds: `[AGENT, wallet]`.
pub const AGENT: &[u8] = b"sap_agent";

/// Per-agent hot-path metrics. Seeds: `[STATS, agent_pda]`.
pub const STATS: &[u8] = b"sap_stats";

/// Per-agent on-chain pricing copy. Seeds: `[PRICING, agent_pda]`.
pub const PRICING: &[u8] = b"sap_pricing";

/// Reviewer feedback (one per (agent, reviewer)). Seeds: `[FEEDBACK, agent_pda, reviewer]`.
pub const FEEDBACK: &[u8] = b"sap_feedback";

/// Capability discovery index. Seeds: `[CAP_IDX, sha256(capability_id)]`.
pub const CAP_IDX: &[u8] = b"sap_cap_idx";

/// Protocol discovery index. Seeds: `[PROTO_IDX, sha256(protocol_id)]`.
pub const PROTO_IDX: &[u8] = b"sap_proto_idx";

/// x402 escrow v2. Seeds: `[ESCROW_V2, agent_pda, depositor, nonce(u64 LE)]`.
pub const ESCROW_V2: &[u8] = b"sap_escrow_v2";

/// Dispute record. Seeds: `[DISPUTE, escrow_v2_pda, …]`.
pub const DISPUTE: &[u8] = b"sap_dispute";

/// Pending settlement (dispute window). Seeds: `[PENDING, escrow_v2_pda, settlement_index]`.
pub const PENDING: &[u8] = b"sap_pending";

/// Receipt batch (merkle-root commit). Seeds: `[RECEIPT, escrow_v2_pda, batch_index]`.
pub const RECEIPT: &[u8] = b"sap_receipt";

/// Agent collateral stake. Seeds: `[STAKE, agent_pda]`.
pub const STAKE: &[u8] = b"sap_stake";

/// Subscription payment channel. Seeds: `[SUB, agent_pda, subscriber]`.
pub const SUB: &[u8] = b"sap_sub";

/// Counter shard (parallelized counters). Seeds: `[SHARD, base_pda, shard_index]`.
pub const SHARD: &[u8] = b"sap_shard";

/// Index overflow page. Seeds: `[IDX_PAGE, index_pda, page_index]`.
pub const IDX_PAGE: &[u8] = b"sap_idx_page";

/// Tool descriptor. Seeds: `[TOOL, agent_pda, tool_hash]`.
pub const TOOL: &[u8] = b"sap_tool";

/// Tool category index. Seeds: `[TOOL_CAT, category_hash]`.
pub const TOOL_CAT: &[u8] = b"sap_tool_cat";

/// Agent attestation. Seeds: `[ATTEST, agent_pda, attester]`.
pub const ATTEST: &[u8] = b"sap_attest";

/// Encrypted memory vault. Seeds: `[VAULT, agent_pda]`.
pub const VAULT: &[u8] = b"sap_vault";

/// Session ledger. Seeds: `[SESSION, vault_pda, session_hash]`.
pub const SESSION: &[u8] = b"sap_session";

/// Epoch scan page. Seeds: `[EPOCH, session_pda, epoch_index(u32 LE)]`.
pub const EPOCH: &[u8] = b"sap_epoch";

/// Session checkpoint. Seeds: `[CHECKPOINT, session_pda, checkpoint_index]`.
pub const CHECKPOINT: &[u8] = b"sap_checkpoint";

/// Ledger 4KB ring-buffer page. Seeds: `[LEDGER, session_pda]` (live module).
pub const LEDGER: &[u8] = b"sap_ledger";

/// LedgerPage seal target. Seeds: `[PAGE, ledger_pda, page_index]` (live module).
pub const PAGE: &[u8] = b"sap_page";

// ── legacy-memory gated ──

/// Legacy memory entry. Seeds: `[MEMORY, agent_pda, entry_hash]`.
#[cfg(feature = "legacy-memory")]
pub const MEMORY: &[u8] = b"sap_memory";

/// Legacy memory chunk. Seeds: `[MEM_CHUNK, memory_entry_pda, chunk_index]`.
#[cfg(feature = "legacy-memory")]
pub const MEM_CHUNK: &[u8] = b"sap_mem_chunk";

/// Legacy plugin slot. Seeds: `[PLUGIN, agent_pda, plugin_type]`.
#[cfg(feature = "legacy-memory")]
pub const PLUGIN: &[u8] = b"sap_plugin";

/// Legacy memory buffer. Seeds: `[BUFFER, agent_pda, buffer_id]`.
#[cfg(feature = "legacy-memory")]
pub const BUFFER: &[u8] = b"sap_buffer";

/// Legacy digest. Seeds: `[DIGEST, agent_pda]`.
#[cfg(feature = "legacy-memory")]
pub const DIGEST: &[u8] = b"sap_digest";

#[cfg(test)]
mod tests {
    use super::*;

    /// Wire-compat gate: byte-identical to the mainnet v1.0.3 literals.
    /// If any assertion fails, a seed literal was edited — that change would
    /// derive different PDAs and orphan live accounts. REVERT.
    #[test]
    fn seeds_are_frozen_protocol_surfaces() {
        assert_eq!(GLOBAL, b"sap_global");
        assert_eq!(AGENT, b"sap_agent");
        assert_eq!(STATS, b"sap_stats");
        assert_eq!(PRICING, b"sap_pricing");
        assert_eq!(FEEDBACK, b"sap_feedback");
        assert_eq!(CAP_IDX, b"sap_cap_idx");
        assert_eq!(PROTO_IDX, b"sap_proto_idx");
        assert_eq!(ESCROW_V2, b"sap_escrow_v2");
        assert_eq!(DISPUTE, b"sap_dispute");
        assert_eq!(PENDING, b"sap_pending");
        assert_eq!(RECEIPT, b"sap_receipt");
        assert_eq!(STAKE, b"sap_stake");
        assert_eq!(SUB, b"sap_sub");
        assert_eq!(SHARD, b"sap_shard");
        assert_eq!(IDX_PAGE, b"sap_idx_page");
        assert_eq!(TOOL, b"sap_tool");
        assert_eq!(TOOL_CAT, b"sap_tool_cat");
        assert_eq!(ATTEST, b"sap_attest");
        assert_eq!(VAULT, b"sap_vault");
        assert_eq!(SESSION, b"sap_session");
        assert_eq!(EPOCH, b"sap_epoch");
        assert_eq!(CHECKPOINT, b"sap_checkpoint");
        assert_eq!(LEDGER, b"sap_ledger");
        assert_eq!(PAGE, b"sap_page");
    }

    #[cfg(feature = "legacy-memory")]
    #[test]
    fn legacy_seeds_are_frozen() {
        assert_eq!(MEMORY, b"sap_memory");
        assert_eq!(MEM_CHUNK, b"sap_mem_chunk");
        assert_eq!(PLUGIN, b"sap_plugin");
        assert_eq!(BUFFER, b"sap_buffer");
        assert_eq!(DIGEST, b"sap_digest");
    }
}
