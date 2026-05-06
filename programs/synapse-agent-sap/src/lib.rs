use anchor_lang::prelude::*;
#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

pub mod state;
pub mod instructions;
pub mod events;
pub mod errors;
pub mod validator;

use instructions::*;
use state::*;

declare_id!("SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ");

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Synapse Agent Protocol (SAP v2)",
    project_url: "https://oobeprotocol.ai",
    contacts: "email:security@oobeprotocol.ai",
    policy: "https://oobeprotocol.ai/security",
    preferred_languages: "en,it",
    source_code: "https://github.com/oobe-protocol/synapse-agent-sap",
    auditors: "Internal"
}

/// SAP v2 — Onchain agent identity, reputation, and discovery protocol.
#[program]
pub mod synapse_agent_sap {
    use super::*;

    // ═══════════════════════════════════════════════
    //  Global Registry
    // ═══════════════════════════════════════════════

    /// Init global registry singleton. Must be called first.
    pub fn initialize_global(ctx: Context<InitializeGlobalAccountConstraints>) -> Result<()> {
        instructions::global::handle_initialize_global(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Agent Lifecycle
    // ═══════════════════════════════════════════════

    /// Register a new agent PDA with metadata.
    pub fn register_agent(
        ctx: Context<RegisterAgentAccountConstraints>,
        name: String,
        description: String,
        capabilities: Vec<Capability>,
        pricing: Vec<PricingTier>,
        protocols: Vec<String>,
        agent_id: Option<String>,
        agent_uri: Option<String>,
        x402_endpoint: Option<String>,
    ) -> Result<()> {
        instructions::agent::register_handler(
            ctx,
            name,
            description,
            capabilities,
            pricing,
            protocols,
            agent_id,
            agent_uri,
            x402_endpoint,
        )
    }

    /// Partial agent metadata update. None = unchanged.
    pub fn update_agent(
        ctx: Context<UpdateAgentAccountConstraints>,
        name: Option<String>,
        description: Option<String>,
        capabilities: Option<Vec<Capability>>,
        pricing: Option<Vec<PricingTier>>,
        protocols: Option<Vec<String>>,
        agent_id: Option<String>,
        agent_uri: Option<String>,
        x402_endpoint: Option<String>,
    ) -> Result<()> {
        instructions::agent::update_handler(
            ctx,
            name,
            description,
            capabilities,
            pricing,
            protocols,
            agent_id,
            agent_uri,
            x402_endpoint,
        )
    }

    /// Set is_active=false. Index entries filtered on read.
    pub fn deactivate_agent(ctx: Context<DeactivateAgentAccountConstraints>) -> Result<()> {
        instructions::agent::deactivate_handler(ctx)
    }

    /// Reactivate a previously deactivated agent.
    pub fn reactivate_agent(ctx: Context<ReactivateAgentAccountConstraints>) -> Result<()> {
        instructions::agent::reactivate_handler(ctx)
    }

    /// Close agent PDA. Rent → wallet. Remove from indexes first.
    pub fn close_agent(ctx: Context<CloseAgentAccountConstraints>) -> Result<()> {
        instructions::agent::close_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Trustless Reputation (Feedback)
    // ═══════════════════════════════════════════════

    /// Leave feedback for agent. One per (agent, reviewer). Score 0-1000.
    pub fn give_feedback(
        ctx: Context<GiveFeedbackAccountConstraints>,
        score: u16,
        tag: String,
        comment_hash: Option<[u8; 32]>,
    ) -> Result<()> {
        instructions::feedback::give_handler(ctx, score, tag, comment_hash)
    }

    /// Update feedback. Original reviewer only.
    pub fn update_feedback(
        ctx: Context<UpdateFeedbackAccountConstraints>,
        new_score: u16,
        new_tag: Option<String>,
        comment_hash: Option<[u8; 32]>,
    ) -> Result<()> {
        instructions::feedback::handle_update_feedback(ctx, new_score, new_tag, comment_hash)
    }

    /// Mark feedback as revoked. Excluded from reputation.
    pub fn revoke_feedback(ctx: Context<RevokeFeedbackAccountConstraints>) -> Result<()> {
        instructions::feedback::revoke_handler(ctx)
    }

    /// Close revoked feedback PDA. Rent → reviewer.
    pub fn close_feedback(ctx: Context<CloseFeedbackAccountConstraints>) -> Result<()> {
        instructions::feedback::close_feedback_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Indexing System (Scalable Discovery)
    //  ACL: all index mutations require agent ownership
    // ═══════════════════════════════════════════════

    /// Init capability index + add caller’s agent. Client computes SHA-256(capability_id).
    pub fn init_capability_index(
        ctx: Context<InitCapabilityIndexAccountConstraints>,
        capability_id: String,
        capability_hash: [u8; 32],
    ) -> Result<()> {
        instructions::indexing::init_capability_handler(ctx, capability_id, capability_hash)
    }

    /// Add the caller's agent to an existing capability index.
    pub fn add_to_capability_index(
        ctx: Context<AddToCapabilityIndexAccountConstraints>,
        capability_hash: [u8; 32],
    ) -> Result<()> {
        instructions::indexing::add_to_capability_handler(ctx, capability_hash)
    }

    /// Remove the caller's agent from a capability index.
    pub fn remove_from_capability_index(
        ctx: Context<RemoveFromCapabilityIndexAccountConstraints>,
        capability_hash: [u8; 32],
    ) -> Result<()> {
        instructions::indexing::remove_from_capability_handler(ctx, capability_hash)
    }

    /// Create a new protocol index and add the caller's agent.
    pub fn init_protocol_index(
        ctx: Context<InitProtocolIndexAccountConstraints>,
        protocol_id: String,
        protocol_hash: [u8; 32],
    ) -> Result<()> {
        instructions::indexing::init_protocol_handler(ctx, protocol_id, protocol_hash)
    }

    /// Add the caller's agent to an existing protocol index.
    pub fn add_to_protocol_index(
        ctx: Context<AddToProtocolIndexAccountConstraints>,
        protocol_hash: [u8; 32],
    ) -> Result<()> {
        instructions::indexing::add_to_protocol_handler(ctx, protocol_hash)
    }

    /// Remove the caller's agent from a protocol index.
    pub fn remove_from_protocol_index(
        ctx: Context<RemoveFromProtocolIndexAccountConstraints>,
        protocol_hash: [u8; 32],
    ) -> Result<()> {
        instructions::indexing::remove_from_protocol_handler(ctx, protocol_hash)
    }

    /// Close empty capability index PDA. Remove all agents first.
    pub fn close_capability_index(
        ctx: Context<CloseCapabilityIndexAccountConstraints>,
        capability_hash: [u8; 32],
    ) -> Result<()> {
        instructions::indexing::close_capability_index_handler(ctx, capability_hash)
    }

    /// Close empty protocol index PDA. Remove all agents first.
    pub fn close_protocol_index(
        ctx: Context<CloseProtocolIndexAccountConstraints>,
        protocol_hash: [u8; 32],
    ) -> Result<()> {
        instructions::indexing::close_protocol_index_handler(ctx, protocol_hash)
    }

    // ═══════════════════════════════════════════════
    //  Plugin System (Extensible PDAs)
    //  [LEGACY — gated behind "legacy-memory" feature]
    // ═══════════════════════════════════════════════

    /// Register plugin slot. type: 0=Memory..5=Custom.
    #[cfg(feature = "legacy-memory")]
    pub fn register_plugin(
        ctx: Context<RegisterPluginAccountConstraints>,
        plugin_type: u8,
    ) -> Result<()> {
        instructions::plugin::handle_register_plugin(ctx, plugin_type)
    }

    /// Close plugin PDA. Removes from agent.active_plugins.
    #[cfg(feature = "legacy-memory")]
    pub fn close_plugin(ctx: Context<ClosePluginAccountConstraints>) -> Result<()> {
        instructions::plugin::close_plugin_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Memory Layer (Hybrid IPFS + Onchain)
    //  [LEGACY — gated behind "legacy-memory" feature]
    // ═══════════════════════════════════════════════

    /// Store a memory entry (metadata + optional IPFS pointer).
    #[cfg(feature = "legacy-memory")]
    pub fn store_memory(
        ctx: Context<StoreMemoryAccountConstraints>,
        entry_hash: [u8; 32],
        content_type: String,
        ipfs_cid: Option<String>,
        total_size: u32,
    ) -> Result<()> {
        instructions::memory::store_handler(ctx, entry_hash, content_type, ipfs_cid, total_size)
    }

    /// Append onchain chunk to memory entry. Max 900B.
    #[cfg(feature = "legacy-memory")]
    pub fn append_memory_chunk(
        ctx: Context<AppendMemoryChunkAccountConstraints>,
        chunk_index: u8,
        data: Vec<u8>,
    ) -> Result<()> {
        instructions::memory::append_chunk_handler(ctx, chunk_index, data)
    }

    /// Close a memory entry PDA (rent returned to wallet).
    #[cfg(feature = "legacy-memory")]
    pub fn close_memory_entry(ctx: Context<CloseMemoryEntryAccountConstraints>) -> Result<()> {
        instructions::memory::close_memory_entry_handler(ctx)
    }

    /// Close a memory chunk PDA (rent returned to wallet).
    #[cfg(feature = "legacy-memory")]
    pub fn close_memory_chunk(ctx: Context<CloseMemoryChunkAccountConstraints>) -> Result<()> {
        instructions::memory::close_memory_chunk_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Memory Vault (Encrypted TX Inscriptions)
    //
    //  Zero-rent encrypted memory via transaction
    //  inscriptions.  Data lives in TX logs forever;
    //  only compact index PDAs pay rent.
    // ═══════════════════════════════════════════════

    /// Init vault. vault_nonce = PBKDF2 salt. Key derived client-side, never onchain.
    pub fn init_vault(
        ctx: Context<InitVaultAccountConstraints>,
        vault_nonce: [u8; 32],
    ) -> Result<()> {
        instructions::vault::init_vault_handler(ctx, vault_nonce)
    }

    /// Open session. session_hash = SHA-256 of deterministic ID.
    pub fn open_session(
        ctx: Context<OpenSessionAccountConstraints>,
        session_hash: [u8; 32],
    ) -> Result<()> {
        instructions::vault::open_session_handler(ctx, session_hash)
    }

    /// Inscribe AES-256-GCM ciphertext to TX log. Zero rent. Fragment if >750B.
    pub fn inscribe_memory(
        ctx: Context<InscribeMemoryAccountConstraints>,
        sequence: u32,
        encrypted_data: Vec<u8>,
        nonce: [u8; 12],
        content_hash: [u8; 32],
        total_fragments: u8,
        fragment_index: u8,
        compression: u8,
        epoch_index: u32,
    ) -> Result<()> {
        instructions::vault::inscribe_memory_handler(
            ctx,
            sequence,
            encrypted_data,
            nonce,
            content_hash,
            total_fragments,
            fragment_index,
            compression,
            epoch_index,
        )
    }

    /// Close a session — no more inscriptions allowed after this.
    pub fn close_session(ctx: Context<CloseSessionAccountConstraints>) -> Result<()> {
        instructions::vault::close_session_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Vault Lifecycle (Close / Rotate / Delegate)
    // ═══════════════════════════════════════════════

    /// Close the MemoryVault PDA and reclaim rent.
    pub fn close_vault(ctx: Context<CloseVaultAccountConstraints>) -> Result<()> {
        instructions::vault::close_vault_handler(ctx)
    }

    /// Close session PDA. Must be closed (via close_session) first.
    pub fn close_session_pda(ctx: Context<CloseSessionPdaAccountConstraints>) -> Result<()> {
        instructions::vault::close_session_pda_handler(ctx)
    }

    /// Close an EpochPage PDA and reclaim rent.
    pub fn close_epoch_page(
        ctx: Context<CloseEpochPageAccountConstraints>,
        epoch_index: u32,
    ) -> Result<()> {
        instructions::vault::close_epoch_page_handler(ctx, epoch_index)
    }

    /// Rotate vault PBKDF2 nonce. Old nonce emitted for historical decryption.
    pub fn rotate_vault_nonce(
        ctx: Context<RotateVaultNonceAccountConstraints>,
        new_nonce: [u8; 32],
    ) -> Result<()> {
        instructions::vault::rotate_vault_nonce_handler(ctx, new_nonce)
    }

    /// Add delegate (hot wallet). Bitmask: 1=inscribe, 2=close, 4=open. expires_at: 0=never.
    pub fn add_vault_delegate(
        ctx: Context<AddVaultDelegateAccountConstraints>,
        permissions: u8,
        expires_at: i64,
    ) -> Result<()> {
        instructions::vault::add_vault_delegate_handler(ctx, permissions, expires_at)
    }

    /// Revoke a delegate's authorization (closes PDA, returns rent).
    pub fn revoke_vault_delegate(ctx: Context<RevokeVaultDelegateAccountConstraints>) -> Result<()> {
        instructions::vault::revoke_vault_delegate_handler(ctx)
    }

    /// Inscribe via authorized delegate. Same as inscribe_memory, delegate signs.
    pub fn inscribe_memory_delegated(
        ctx: Context<InscribeMemoryDelegatedAccountConstraints>,
        sequence: u32,
        encrypted_data: Vec<u8>,
        nonce: [u8; 12],
        content_hash: [u8; 32],
        total_fragments: u8,
        fragment_index: u8,
        compression: u8,
        epoch_index: u32,
    ) -> Result<()> {
        instructions::vault::inscribe_memory_delegated_handler(
            ctx,
            sequence,
            encrypted_data,
            nonce,
            content_hash,
            total_fragments,
            fragment_index,
            compression,
            epoch_index,
        )
    }

    /// DX-first inscription (4 args vs 8). Single fragment, no compression/epochs.
    pub fn compact_inscribe(
        ctx: Context<CompactInscribeAccountConstraints>,
        sequence: u32,
        encrypted_data: Vec<u8>,
        nonce: [u8; 12],
        content_hash: [u8; 32],
    ) -> Result<()> {
        instructions::vault::compact_inscribe_handler(
            ctx,
            sequence,
            encrypted_data,
            nonce,
            content_hash,
        )
    }

    // ═══════════════════════════════════════════════
    //  Tool Schema Registry (Onchain Typed Tools)
    //
    //  Each tool an agent exposes gets a ToolDescriptor PDA
    //  with compact metadata + schema hashes.  Full JSON schemas
    //  are inscribed in TX logs (zero rent, permanent).
    //  Enables verifiable, discoverable, typed API surfaces.
    // ═══════════════════════════════════════════════

    /// Publish tool PDA. tool_name_hash = sha256(tool_name).
    pub fn publish_tool(
        ctx: Context<PublishToolAccountConstraints>,
        tool_name: String,
        tool_name_hash: [u8; 32],
        protocol_hash: [u8; 32],
        description_hash: [u8; 32],
        input_schema_hash: [u8; 32],
        output_schema_hash: [u8; 32],
        http_method: u8,
        category: u8,
        params_count: u8,
        required_params: u8,
        is_compound: bool,
    ) -> Result<()> {
        instructions::tools::publish_tool_handler(
            ctx,
            tool_name,
            tool_name_hash,
            protocol_hash,
            description_hash,
            input_schema_hash,
            output_schema_hash,
            http_method,
            category,
            params_count,
            required_params,
            is_compound,
        )
    }

    /// Inscribe JSON schema to TX log. schema_type: 0=input, 1=output, 2=desc.
    pub fn inscribe_tool_schema(
        ctx: Context<InscribeToolSchemaAccountConstraints>,
        schema_type: u8,
        schema_data: Vec<u8>,
        schema_hash: [u8; 32],
        compression: u8,
    ) -> Result<()> {
        instructions::tools::inscribe_tool_schema_handler(
            ctx,
            schema_type,
            schema_data,
            schema_hash,
            compression,
        )
    }

    /// Update tool schema hashes + bump version. None = unchanged.
    pub fn update_tool(
        ctx: Context<UpdateToolAccountConstraints>,
        description_hash: Option<[u8; 32]>,
        input_schema_hash: Option<[u8; 32]>,
        output_schema_hash: Option<[u8; 32]>,
        http_method: Option<u8>,
        category: Option<u8>,
        params_count: Option<u8>,
        required_params: Option<u8>,
    ) -> Result<()> {
        instructions::tools::update_tool_handler(
            ctx,
            description_hash,
            input_schema_hash,
            output_schema_hash,
            http_method,
            category,
            params_count,
            required_params,
        )
    }

    /// Deactivate a tool (still discoverable but marked unavailable).
    pub fn deactivate_tool(ctx: Context<DeactivateToolAccountConstraints>) -> Result<()> {
        instructions::tools::deactivate_tool_handler(ctx)
    }

    /// Reactivate a previously deactivated tool.
    pub fn reactivate_tool(ctx: Context<ReactivateToolAccountConstraints>) -> Result<()> {
        instructions::tools::reactivate_tool_handler(ctx)
    }

    /// Close a tool PDA (rent returned to wallet).
    pub fn close_tool(ctx: Context<CloseToolAccountConstraints>) -> Result<()> {
        instructions::tools::close_tool_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Session Checkpoints (Fast-Sync Snapshots)
    //
    //  Periodic snapshots of merkle_root + counters.
    //  Enables fast sync, parallel verification,
    //  and deterministic recovery points.
    // ═══════════════════════════════════════════════

    /// Checkpoint: snapshot merkle_root + counters for fast sync.
    pub fn create_session_checkpoint(
        ctx: Context<CreateSessionCheckpointAccountConstraints>,
        checkpoint_index: u32,
    ) -> Result<()> {
        instructions::tools::create_session_checkpoint_handler(ctx, checkpoint_index)
    }

    /// Close a checkpoint PDA (rent returned to wallet).
    pub fn close_checkpoint(
        ctx: Context<CloseCheckpointAccountConstraints>,
        checkpoint_index: u32,
    ) -> Result<()> {
        instructions::tools::close_checkpoint_handler(ctx, checkpoint_index)
    }

    // ═══════════════════════════════════════════════
    //  Agent Attestation — Web of Trust
    //
    //  Third-party verifiable trust signals.
    //  Anyone can attest for any agent (one per pair).
    //  Trust from WHO attests, not the attestation itself.
    //  Lifecycle: create → revoke → close
    // ═══════════════════════════════════════════════

    /// Create attestation. attestation_type: max 32 chars. expires_at: 0=never.
    pub fn create_attestation(
        ctx: Context<CreateAttestationAccountConstraints>,
        attestation_type: String,
        metadata_hash: [u8; 32],
        expires_at: i64,
    ) -> Result<()> {
        instructions::attestation::create_attestation_handler(
            ctx,
            attestation_type,
            metadata_hash,
            expires_at,
        )
    }

    /// Revoke attestation. Original attester only.
    pub fn revoke_attestation(ctx: Context<RevokeAttestationAccountConstraints>) -> Result<()> {
        instructions::attestation::revoke_attestation_handler(ctx)
    }

    /// Close revoked attestation PDA. Must revoke first.
    pub fn close_attestation(ctx: Context<CloseAttestationAccountConstraints>) -> Result<()> {
        instructions::attestation::close_attestation_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Tool Category Index (Cross-Agent Discovery)
    //
    //  Indexes ToolDescriptor PDAs by category
    //  across all agents.  Enables ecosystem-wide
    //  queries like "show me all Swap tools".
    //  PDA: ["sap_tool_cat", &[category_u8]]
    // ═══════════════════════════════════════════════

    /// Init tool category index. category: 0=Swap..9=Custom.
    pub fn init_tool_category_index(
        ctx: Context<InitToolCategoryIndexAccountConstraints>,
        category: u8,
    ) -> Result<()> {
        instructions::indexing::init_tool_category_index_handler(ctx, category)
    }

    /// Add tool to category index. Verifies tool.category match.
    pub fn add_to_tool_category(
        ctx: Context<AddToToolCategoryAccountConstraints>,
        category: u8,
    ) -> Result<()> {
        instructions::indexing::add_to_tool_category_handler(ctx, category)
    }

    /// Remove a tool from a category index.
    pub fn remove_from_tool_category(
        ctx: Context<RemoveFromToolCategoryAccountConstraints>,
        category: u8,
    ) -> Result<()> {
        instructions::indexing::remove_from_tool_category_handler(ctx, category)
    }

    /// Close empty tool category index. Remove all tools first.
    pub fn close_tool_category_index(
        ctx: Context<CloseToolCategoryIndexAccountConstraints>,
        category: u8,
    ) -> Result<()> {
        instructions::indexing::close_tool_category_index_handler(ctx, category)
    }

    // ═══════════════════════════════════════════════
    //  Memory Buffer (Onchain Readable Session Cache)
    //  [LEGACY — gated behind "legacy-memory" feature]
    //
    //  Complements TX log inscriptions with onchain
    //  readable PDA buffers.  Data accessible via any
    //  free RPC getAccountInfo() — no archival access
    //  needed.  Uses dynamic realloc for minimal rent.
    // ═══════════════════════════════════════════════

    /// Create buffer page PDA. ≈0.001 SOL rent, reclaimable.
    #[cfg(feature = "legacy-memory")]
    pub fn create_buffer(
        ctx: Context<CreateBufferAccountConstraints>,
        page_index: u32,
    ) -> Result<()> {
        instructions::buffer::create_buffer_handler(ctx, page_index)
    }

    /// Append data to buffer page. ≤750B per call, uses realloc.
    #[cfg(feature = "legacy-memory")]
    pub fn append_buffer(
        ctx: Context<AppendBufferAccountConstraints>,
        page_index: u32,
        data: Vec<u8>,
    ) -> Result<()> {
        instructions::buffer::append_buffer_handler(ctx, page_index, data)
    }

    /// Close a buffer page, reclaim ALL accumulated rent.
    #[cfg(feature = "legacy-memory")]
    pub fn close_buffer(
        ctx: Context<CloseBufferAccountConstraints>,
        page_index: u32,
    ) -> Result<()> {
        instructions::buffer::close_buffer_handler(ctx, page_index)
    }

    // ═══════════════════════════════════════════════
    //  Memory Digest — Proof-of-Memory Protocol
    //  [LEGACY — gated behind "legacy-memory" feature]
    //
    //  Fixed-size PDA (~0.002 SOL, never grows).
    //  Everything onchain: data in TX logs, proof in PDA.
    //  Each write costs ONLY the TX fee (~0.000005 SOL).
    //  10K entries × 1KB = ~0.052 SOL total.
    //  No offchain dependency.  100% immutable.
    // ═══════════════════════════════════════════════

    /// Init MemoryDigest PDA. Fixed ~0.002 SOL, never grows.
    #[cfg(feature = "legacy-memory")]
    pub fn init_digest(ctx: Context<InitDigestAccountConstraints>) -> Result<()> {
        instructions::digest::init_digest_handler(ctx)
    }

    /// Post digest proof (hash only). Zero additional rent.
    #[cfg(feature = "legacy-memory")]
    pub fn post_digest(
        ctx: Context<PostDigestAccountConstraints>,
        content_hash: [u8; 32],
        data_size: u32,
    ) -> Result<()> {
        instructions::digest::post_digest_handler(ctx, content_hash, data_size)
    }

    /// Inscribe data to TX log + update PDA proof. Primary write. Zero rent.
    #[cfg(feature = "legacy-memory")]
    pub fn inscribe_to_digest(
        ctx: Context<InscribeToDigestAccountConstraints>,
        data: Vec<u8>,
        content_hash: [u8; 32],
    ) -> Result<()> {
        instructions::digest::inscribe_to_digest_handler(ctx, data, content_hash)
    }

    /// Update optional offchain storage pointer (IPFS CID, Arweave TX, etc.).
    #[cfg(feature = "legacy-memory")]
    pub fn update_digest_storage(
        ctx: Context<UpdateDigestStorageAccountConstraints>,
        storage_ref: [u8; 32],
        storage_type: u8,
    ) -> Result<()> {
        instructions::digest::update_digest_storage_handler(ctx, storage_ref, storage_type)
    }

    /// Close a digest PDA, reclaim all rent.
    #[cfg(feature = "legacy-memory")]
    pub fn close_digest(ctx: Context<CloseDigestAccountConstraints>) -> Result<()> {
        instructions::digest::close_digest_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Memory Ledger — Unified Onchain Memory
    //
    //  THE RECOMMENDED MEMORY SYSTEM.
    //  Fixed PDA with 4KB ring buffer (~0.032 SOL).
    //  Hot path:  getAccountInfo() → latest msgs → FREE
    //  Cold path: getSignatures + getTx → full history
    //  One instruction writes both ring + TX log.
    // ═══════════════════════════════════════════════

    /// Init MemoryLedger with 4KB ring. Fixed ~0.032 SOL.
    pub fn init_ledger(ctx: Context<InitLedgerAccountConstraints>) -> Result<()> {
        instructions::ledger::init_ledger_handler(ctx)
    }

    /// Write to TX log (permanent) + ring buffer (instant read). Cost = TX fee.
    pub fn write_ledger(
        ctx: Context<WriteLedgerAccountConstraints>,
        data: Vec<u8>,
        content_hash: [u8; 32],
    ) -> Result<()> {
        instructions::ledger::write_ledger_handler(ctx, data, content_hash)
    }

    /// Seal ring into permanent LedgerPage. Write-once, no close exists.
    pub fn seal_ledger(ctx: Context<SealLedgerAccountConstraints>) -> Result<()> {
        instructions::ledger::seal_ledger_handler(ctx)
    }

    /// Close ledger PDA. Sealed pages remain permanent.
    pub fn close_ledger(ctx: Context<CloseLedgerAccountConstraints>) -> Result<()> {
        instructions::ledger::close_ledger_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  x402 Escrow V2 — Triple-Mode Settlement
    //
    //  Multi-escrow with nonce, three settlement security
    //  modes: SelfReport, CoSigned, DisputeWindow.
    //  Supersedes v1 escrow for new integrations.
    // ═══════════════════════════════════════════════

    /// Create V2 escrow with settlement security mode.
    pub fn create_escrow_v2<'info>(
        ctx: Context<'info, CreateEscrowV2AccountConstraints<'info>>,
        escrow_nonce: u64,
        price_per_call: u64,
        max_calls: u64,
        initial_deposit: u64,
        expires_at: i64,
        volume_curve: Vec<VolumeCurveBreakpoint>,
        token_mint: Option<Pubkey>,
        token_decimals: u8,
        settlement_security: u8,
        dispute_window_slots: u64,
        co_signer: Option<Pubkey>,
        arbiter: Option<Pubkey>,
    ) -> Result<()> {
        instructions::escrow_v2::create_escrow_v2_handler(
            ctx, escrow_nonce, price_per_call, max_calls, initial_deposit,
            expires_at, volume_curve, token_mint, token_decimals,
            settlement_security, dispute_window_slots, co_signer, arbiter,
        )
    }

    /// Deposit into V2 escrow.
    pub fn deposit_escrow_v2<'info>(
        ctx: Context<'info, DepositEscrowV2AccountConstraints<'info>>,
        escrow_nonce: u64,
        amount: u64,
    ) -> Result<()> {
        instructions::escrow_v2::deposit_escrow_v2_handler(ctx, escrow_nonce, amount)
    }

    /// Agent settles calls via V2 escrow. Mode-dispatched.
    pub fn settle_calls_v2<'info>(
        ctx: Context<'info, SettleCallsV2AccountConstraints<'info>>,
        escrow_nonce: u64,
        calls_to_settle: u64,
        service_hash: [u8; 32],
    ) -> Result<()> {
        instructions::escrow_v2::settle_calls_v2_handler(ctx, escrow_nonce, calls_to_settle, service_hash)
    }

    /// Create PendingSettlement PDA (DisputeWindow mode).
    pub fn create_pending_settlement(
        ctx: Context<CreatePendingSettlementAccountConstraints>,
        settlement_index: u64,
        calls_to_settle: u64,
        amount: u64,
        service_hash: [u8; 32],
        receipt_merkle_root: [u8; 32],
    ) -> Result<()> {
        instructions::escrow_v2::create_pending_settlement_handler(ctx, settlement_index, calls_to_settle, amount, service_hash, receipt_merkle_root)
    }

    /// Finalize settlement after dispute window. Permissionless crank.
    pub fn finalize_settlement<'info>(
        ctx: Context<'info, FinalizeSettlementAccountConstraints<'info>>,
    ) -> Result<()> {
        instructions::escrow_v2::finalize_settlement_handler(ctx)
    }

    /// Withdraw from V2 escrow (available balance only).
    pub fn withdraw_escrow_v2<'info>(
        ctx: Context<'info, WithdrawEscrowV2AccountConstraints<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::escrow_v2::withdraw_escrow_v2_handler(ctx, amount)
    }

    /// Close empty V2 escrow. No pending settlements allowed.
    pub fn close_escrow_v2(ctx: Context<CloseEscrowV2AccountConstraints>) -> Result<()> {
        instructions::escrow_v2::close_escrow_v2_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Dispute Resolution (v0.7 — Receipt-Based)
    //
    //  Depositor files dispute → agent proves via
    //  receipt merkle proofs → auto-resolved on-chain.
    //  No arbiter required.
    // ═══════════════════════════════════════════════

    /// Depositor files dispute on a pending settlement.
    pub fn file_dispute(
        ctx: Context<FileDisputeAccountConstraints>,
        evidence_hash: [u8; 32],
        dispute_type: u8,
    ) -> Result<()> {
        instructions::dispute::file_dispute_handler(ctx, evidence_hash, dispute_type)
    }

    /// Agent submits counter-evidence for a dispute.
    pub fn submit_agent_evidence(
        ctx: Context<SubmitAgentEvidenceAccountConstraints>,
        evidence_hash: [u8; 32],
    ) -> Result<()> {
        instructions::dispute::submit_agent_evidence_handler(ctx, evidence_hash)
    }

    /// Close finalized dispute PDA. Depositor reclaims rent.
    pub fn close_dispute(ctx: Context<CloseDisputeAccountConstraints>) -> Result<()> {
        instructions::dispute::close_dispute_handler(ctx)
    }

    /// Close finalized pending settlement PDA. Reclaim rent.
    pub fn close_pending_settlement(ctx: Context<ClosePendingSettlementAccountConstraints>) -> Result<()> {
        instructions::dispute::close_pending_settlement_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Receipt Batch System (v0.7)
    //
    //  Agent inscribes merkle roots of dual-signed
    //  call receipts. During disputes, agent proves
    //  delivery via merkle inclusion proofs.
    // ═══════════════════════════════════════════════

    /// Agent commits a batch of receipt merkle roots.
    pub fn inscribe_receipt_batch(
        ctx: Context<InscribeReceiptBatchAccountConstraints>,
        batch_index: u32,
        merkle_root: [u8; 32],
        call_count: u32,
        period_start: i64,
        period_end: i64,
    ) -> Result<()> {
        instructions::receipt::inscribe_receipt_batch_handler(ctx, batch_index, merkle_root, call_count, period_start, period_end)
    }

    /// Agent submits receipt proofs during dispute resolution.
    pub fn submit_receipt_proof(
        ctx: Context<SubmitReceiptProofAccountConstraints>,
        receipt_hashes: Vec<[u8; 32]>,
        merkle_proofs: Vec<Vec<[u8; 32]>>,
    ) -> Result<()> {
        instructions::receipt::submit_receipt_proof_handler(ctx, receipt_hashes, merkle_proofs)
    }

    /// Permissionless auto-resolution after proof deadline.
    pub fn auto_resolve_dispute<'info>(
        ctx: Context<'info, AutoResolveDisputeAccountConstraints<'info>>,
    ) -> Result<()> {
        instructions::receipt::auto_resolve_dispute_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Agent Staking — Collateralized Trust
    //
    //  Agents deposit SOL collateral.  Stake can be
    //  slashed during dispute resolution as penalty.
    //  7-day cooldown on unstaking for safety.
    // ═══════════════════════════════════════════════

    /// Init agent stake PDA with initial deposit.
    pub fn init_stake(
        ctx: Context<InitStakeAccountConstraints>,
        initial_deposit: u64,
    ) -> Result<()> {
        instructions::staking::init_stake_handler(ctx, initial_deposit)
    }

    /// Deposit more SOL into agent stake.
    pub fn deposit_stake(
        ctx: Context<DepositStakeAccountConstraints>,
        amount: u64,
    ) -> Result<()> {
        instructions::staking::deposit_stake_handler(ctx, amount)
    }

    /// Request unstake — starts 7-day cooldown. Supports partial unstake.
    pub fn request_unstake(ctx: Context<RequestUnstakeAccountConstraints>, amount: u64) -> Result<()> {
        instructions::staking::request_unstake_handler(ctx, amount)
    }

    /// Complete unstake after cooldown period.
    pub fn complete_unstake(ctx: Context<CompleteUnstakeAccountConstraints>) -> Result<()> {
        instructions::staking::complete_unstake_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Subscriptions — Recurring Payment Channels
    //
    //  Depositor subscribes to agent with fixed
    //  interval pricing.  Agent claims completed
    //  intervals.  Pro-rata refund on cancellation.
    // ═══════════════════════════════════════════════

    /// Create subscription to agent.
    pub fn create_subscription(
        ctx: Context<CreateSubscriptionAccountConstraints>,
        sub_id: u64,
        price_per_interval: u64,
        billing_interval: u8,
        initial_deposit: u64,
    ) -> Result<()> {
        instructions::subscription::create_subscription_handler(ctx, sub_id, price_per_interval, billing_interval, initial_deposit)
    }

    /// Fund subscription with additional SOL.
    pub fn fund_subscription(
        ctx: Context<FundSubscriptionAccountConstraints>,
        amount: u64,
    ) -> Result<()> {
        instructions::subscription::fund_subscription_handler(ctx, amount)
    }

    /// Permissionless crank: claim completed billing intervals.
    pub fn claim_interval(ctx: Context<ClaimIntervalAccountConstraints>) -> Result<()> {
        instructions::subscription::claim_interval_handler(ctx)
    }

    /// Depositor cancels subscription. Remaining balance refunded.
    pub fn cancel_subscription(ctx: Context<CancelSubscriptionAccountConstraints>) -> Result<()> {
        instructions::subscription::cancel_subscription_handler(ctx)
    }

    /// Close cancelled subscription PDA. Reclaim rent.
    pub fn close_subscription(ctx: Context<CloseSubscriptionAccountConstraints>) -> Result<()> {
        instructions::subscription::close_subscription_handler(ctx)
    }

    // ═══════════════════════════════════════════════
    //  Counter Shards — Parallel Write Throughput
    //
    //  8 shards per counter for 8× throughput.
    //  Used by GlobalRegistry and discovery indexes.
    // ═══════════════════════════════════════════════

    /// Init counter shard for a parent account.
    pub fn init_shard(
        ctx: Context<InitShardAccountConstraints>,
        shard_index: u8,
    ) -> Result<()> {
        instructions::shards::init_shard_handler(ctx, shard_index)
    }

    // ═══════════════════════════════════════════════
    //  Index Pages — Overflow Discovery Indexes
    //
    //  When primary index fills (100 entries),
    //  overflow pages handle additional agents.
    //  Linked pages for unlimited discovery.
    // ═══════════════════════════════════════════════

    /// Init overflow index page.
    pub fn init_index_page(
        ctx: Context<InitIndexPageAccountConstraints>,
        page_index: u8,
    ) -> Result<()> {
        instructions::index_page::init_index_page_handler(ctx, page_index)
    }

    /// Add agent to overflow page.
    pub fn add_to_index_page(
        ctx: Context<AddToIndexPageAccountConstraints>,
        agent_pda: Pubkey,
    ) -> Result<()> {
        instructions::index_page::add_to_index_page_handler(ctx, agent_pda)
    }

    /// Remove agent from overflow page.
    pub fn remove_from_index_page(
        ctx: Context<RemoveFromIndexPageAccountConstraints>,
        agent_pda: Pubkey,
    ) -> Result<()> {
        instructions::index_page::remove_from_index_page_handler(ctx, agent_pda)
    }

    /// Close empty overflow page. Reclaim rent.
    pub fn close_index_page(ctx: Context<CloseIndexPageAccountConstraints>) -> Result<()> {
        instructions::index_page::close_index_page_handler(ctx)
    }
}
