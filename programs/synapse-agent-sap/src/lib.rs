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

    /// Self-report call metrics. No reputation_score effect.
    pub fn report_calls(
        ctx: Context<ReportCallsAccountConstraints>,
        calls_served: u64,
    ) -> Result<()> {
        instructions::agent::report_calls_handler(ctx, calls_served)
    }

    /// Self-report latency & uptime. No reputation_score effect.
    pub fn update_reputation(
        ctx: Context<UpdateReputationAccountConstraints>,
        avg_latency_ms: u32,
        uptime_percent: u8,
    ) -> Result<()> {
        instructions::agent::update_reputation_handler(ctx, avg_latency_ms, uptime_percent)
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

    /// Report tool invocation count (self-reported by agent owner).
    pub fn report_tool_invocations(
        ctx: Context<ReportToolInvocationsAccountConstraints>,
        invocations: u64,
    ) -> Result<()> {
        instructions::tools::report_tool_invocations_handler(ctx, invocations)
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
    //  x402 Escrow Settlement Layer
    //
    //  Pre-funded trustless micropayments between
    //  clients and agents.  Locked price per call,
    //  max calls limit, direct lamport settlement.
    //  PaymentSettledEvent = permanent TX log receipt.
    // ═══════════════════════════════════════════════

    /// Create escrow. Locks price_per_call + optional max_calls. token_mint: None=SOL.
    pub fn create_escrow<'info>(
        ctx: Context<'_, '_, 'info, 'info, CreateEscrowAccountConstraints<'info>>,
        price_per_call: u64,
        max_calls: u64,
        initial_deposit: u64,
        expires_at: i64,
        volume_curve: Vec<VolumeCurveBreakpoint>,
        token_mint: Option<Pubkey>,
        token_decimals: u8,
    ) -> Result<()> {
        instructions::escrow::create_escrow_handler(ctx, price_per_call, max_calls, initial_deposit, expires_at, volume_curve, token_mint, token_decimals)
    }

    /// Deposit additional SOL into an existing escrow.
    pub fn deposit_escrow<'info>(
        ctx: Context<'_, '_, 'info, 'info, DepositEscrowAccountConstraints<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::escrow::deposit_escrow_handler(ctx, amount)
    }

    /// Agent settles calls — claims funds from escrow. service_hash = proof of work.
    pub fn settle_calls<'info>(
        ctx: Context<'_, '_, 'info, 'info, SettleCallsAccountConstraints<'info>>,
        calls_to_settle: u64,
        service_hash: [u8; 32],
    ) -> Result<()> {
        instructions::escrow::settle_calls_handler(ctx, calls_to_settle, service_hash)
    }

    /// Client withdraws from escrow. Withdraws min(amount, balance).
    pub fn withdraw_escrow<'info>(
        ctx: Context<'_, '_, 'info, 'info, WithdrawEscrowAccountConstraints<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::escrow::withdraw_escrow_handler(ctx, amount)
    }

    /// Close empty escrow PDA. Withdraw first.
    pub fn close_escrow(ctx: Context<CloseEscrowAccountConstraints>) -> Result<()> {
        instructions::escrow::close_escrow_handler(ctx)
    }

    /// Batch settle up to 10 settlements in one TX. Volume curve spans batch.
    pub fn settle_batch<'info>(
        ctx: Context<'_, '_, 'info, 'info, SettleBatchAccountConstraints<'info>>,
        settlements: Vec<Settlement>,
    ) -> Result<()> {
        instructions::escrow::settle_batch_handler(ctx, settlements)
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
}
