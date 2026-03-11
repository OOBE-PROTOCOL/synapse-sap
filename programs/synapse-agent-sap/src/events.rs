use anchor_lang::prelude::*;

// ═══════════════════════════════════════════════
//  Agent Lifecycle Events
// ═══════════════════════════════════════════════

#[event]
pub struct RegisteredEvent {
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub name: String,
    pub capabilities: Vec<String>,
    pub timestamp: i64,
}

#[event]
pub struct UpdatedEvent {
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub updated_fields: Vec<String>,
    pub timestamp: i64,
}

#[event]
pub struct DeactivatedEvent {
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct ReactivatedEvent {
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct ClosedEvent {
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Feedback Events
// ═══════════════════════════════════════════════

#[event]
pub struct FeedbackEvent {
    pub agent: Pubkey,
    pub reviewer: Pubkey,
    pub score: u16,
    pub tag: String,
    pub timestamp: i64,
}

#[event]
pub struct FeedbackUpdatedEvent {
    pub agent: Pubkey,
    pub reviewer: Pubkey,
    pub old_score: u16,
    pub new_score: u16,
    pub timestamp: i64,
}

#[event]
pub struct FeedbackRevokedEvent {
    pub agent: Pubkey,
    pub reviewer: Pubkey,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Plugin Events  [LEGACY — gated behind "legacy-memory"]
// ═══════════════════════════════════════════════════

#[cfg(feature = "legacy-memory")]
#[event]
pub struct PluginRegisteredEvent {
    pub agent: Pubkey,
    pub plugin_type: u8,
    pub plugin_pda: Pubkey,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Memory Events  [LEGACY — gated behind "legacy-memory"]
// ═══════════════════════════════════════════════════

#[cfg(feature = "legacy-memory")]
#[event]
pub struct MemoryStoredEvent {
    pub agent: Pubkey,
    pub entry_hash: [u8; 32],
    pub content_type: String,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Reputation Events
// ═══════════════════════════════════════════════

#[event]
pub struct ReputationUpdatedEvent {
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub avg_latency_ms: u32,
    pub uptime_percent: u8,
    pub timestamp: i64,
}

#[event]
pub struct CallsReportedEvent {
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub calls_reported: u64,
    pub total_calls_served: u64,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Memory Vault Events (Transaction Inscriptions)
// ═══════════════════════════════════════════════

#[event]
pub struct VaultInitializedEvent {
    pub agent: Pubkey,
    pub vault: Pubkey,
    pub wallet: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct SessionOpenedEvent {
    pub vault: Pubkey,
    pub session: Pubkey,
    pub session_hash: [u8; 32],
    pub timestamp: i64,
}

/// AES-256-GCM ciphertext inscription. Permanent TX log, zero rent.
/// compression: 0=none, 1=deflate, 2=gzip, 3=brotli.
#[event]
pub struct MemoryInscribedEvent {
    pub vault: Pubkey,
    pub session: Pubkey,
    pub sequence: u32,
    pub epoch_index: u32,
    pub encrypted_data: Vec<u8>,
    pub nonce: [u8; 12],
    pub content_hash: [u8; 32],
    pub total_fragments: u8,
    pub fragment_index: u8,
    pub compression: u8,
    pub data_len: u32,
    pub nonce_version: u32,
    pub timestamp: i64,
}

#[event]
pub struct EpochOpenedEvent {
    pub session: Pubkey,
    pub epoch_page: Pubkey,
    pub epoch_index: u32,
    pub start_sequence: u32,
    pub timestamp: i64,
}

#[event]
pub struct SessionClosedEvent {
    pub vault: Pubkey,
    pub session: Pubkey,
    pub total_inscriptions: u32,
    pub total_bytes: u64,
    pub total_epochs: u32,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Vault Lifecycle Events (Close / Rotate / Delegate)
// ═══════════════════════════════════════════════

#[event]
pub struct VaultClosedEvent {
    pub vault: Pubkey,
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub total_sessions: u32,
    pub total_inscriptions: u64,
    pub timestamp: i64,
}

#[event]
pub struct SessionPdaClosedEvent {
    pub vault: Pubkey,
    pub session: Pubkey,
    pub total_inscriptions: u32,
    pub total_bytes: u64,
    pub timestamp: i64,
}

#[event]
pub struct EpochPageClosedEvent {
    pub session: Pubkey,
    pub epoch_page: Pubkey,
    pub epoch_index: u32,
    pub timestamp: i64,
}

#[event]
pub struct VaultNonceRotatedEvent {
    pub vault: Pubkey,
    pub wallet: Pubkey,
    pub old_nonce: [u8; 32],
    pub new_nonce: [u8; 32],
    pub nonce_version: u32,
    pub timestamp: i64,
}

#[event]
pub struct DelegateAddedEvent {
    pub vault: Pubkey,
    pub delegate: Pubkey,
    pub permissions: u8,
    pub expires_at: i64,
    pub timestamp: i64,
}

#[event]
pub struct DelegateRevokedEvent {
    pub vault: Pubkey,
    pub delegate: Pubkey,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Tool Registry Events (Onchain Schema Registry)
// ═══════════════════════════════════════════════

/// Tool descriptor published. Schema JSON inscribed via ToolSchemaInscribedEvent.
#[event]
pub struct ToolPublishedEvent {
    pub agent: Pubkey,
    pub tool: Pubkey,
    pub tool_name: String,
    pub protocol_hash: [u8; 32],
    pub version: u16,
    pub http_method: u8,
    pub category: u8,
    pub params_count: u8,
    pub required_params: u8,
    pub is_compound: bool,
    pub timestamp: i64,
}

/// Full JSON schema inscribed to TX log. schema_type: 0=input, 1=output, 2=desc.
/// Verify: sha256(schema_data) == schema_hash. compression: 0=none, 1=deflate.
#[event]
pub struct ToolSchemaInscribedEvent {
    pub agent: Pubkey,
    pub tool: Pubkey,
    pub tool_name: String,
    pub schema_type: u8,           // 0=input, 1=output, 2=description
    pub schema_data: Vec<u8>,      // full JSON schema (optionally compressed)
    pub schema_hash: [u8; 32],     // sha256 of uncompressed schema
    pub compression: u8,           // 0=none, 1=deflate
    pub version: u16,
    pub timestamp: i64,
}

/// Tool schema updated, version bumped.
#[event]
pub struct ToolUpdatedEvent {
    pub agent: Pubkey,
    pub tool: Pubkey,
    pub tool_name: String,
    pub old_version: u16,
    pub new_version: u16,
    pub timestamp: i64,
}

/// Tool deactivated.
#[event]
pub struct ToolDeactivatedEvent {
    pub agent: Pubkey,
    pub tool: Pubkey,
    pub tool_name: String,
    pub timestamp: i64,
}

/// Tool reactivated.
#[event]
pub struct ToolReactivatedEvent {
    pub agent: Pubkey,
    pub tool: Pubkey,
    pub tool_name: String,
    pub timestamp: i64,
}

/// Tool PDA closed, rent recovered.
#[event]
pub struct ToolClosedEvent {
    pub agent: Pubkey,
    pub tool: Pubkey,
    pub tool_name: String,
    pub total_invocations: u64,
    pub timestamp: i64,
}

/// Tool invocation counter reported.
#[event]
pub struct ToolInvocationReportedEvent {
    pub agent: Pubkey,
    pub tool: Pubkey,
    pub invocations_reported: u64,
    pub total_invocations: u64,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Checkpoint Events (Fast-Sync Snapshots)
// ═══════════════════════════════════════════════

/// Session checkpoint created.
#[event]
pub struct CheckpointCreatedEvent {
    pub session: Pubkey,
    pub checkpoint: Pubkey,
    pub checkpoint_index: u32,
    pub merkle_root: [u8; 32],
    pub sequence_at: u32,
    pub epoch_at: u32,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  x402 Escrow Events (Settlement Layer)
// ═══════════════════════════════════════════════

/// Escrow created for micropayments.
#[event]
pub struct EscrowCreatedEvent {
    pub escrow: Pubkey,
    pub agent: Pubkey,
    pub depositor: Pubkey,
    pub price_per_call: u64,
    pub max_calls: u64,
    pub initial_deposit: u64,
    pub expires_at: i64,
    pub timestamp: i64,
}

/// Additional escrow deposit.
#[event]
pub struct EscrowDepositedEvent {
    pub escrow: Pubkey,
    pub depositor: Pubkey,
    pub amount: u64,
    pub new_balance: u64,
    pub timestamp: i64,
}

/// Payment receipt. service_hash = sha256 proof of service.
#[event]
pub struct PaymentSettledEvent {
    pub escrow: Pubkey,
    pub agent: Pubkey,
    pub depositor: Pubkey,
    pub calls_settled: u64,
    pub amount: u64,
    pub service_hash: [u8; 32],
    pub total_calls_settled: u64,
    pub remaining_balance: u64,
    pub timestamp: i64,
}

/// Escrow withdrawal.
#[event]
pub struct EscrowWithdrawnEvent {
    pub escrow: Pubkey,
    pub depositor: Pubkey,
    pub amount: u64,
    pub remaining_balance: u64,
    pub timestamp: i64,
}

/// Batch settlement. service_hashes preserved for dispute resolution.
#[event]
pub struct BatchSettledEvent {
    pub escrow: Pubkey,
    pub agent: Pubkey,
    pub depositor: Pubkey,
    pub num_settlements: u8,
    pub total_calls: u64,
    pub total_amount: u64,
    pub service_hashes: Vec<[u8; 32]>,
    pub calls_per_settlement: Vec<u64>,
    pub remaining_balance: u64,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Attestation Events (Web of Trust)
// ═══════════════════════════════════════════════

/// Attestation created.
#[event]
pub struct AttestationCreatedEvent {
    pub agent: Pubkey,
    pub attester: Pubkey,
    pub attestation_type: String,
    pub expires_at: i64,
    pub timestamp: i64,
}

/// Attestation revoked.
#[event]
pub struct AttestationRevokedEvent {
    pub agent: Pubkey,
    pub attester: Pubkey,
    pub attestation_type: String,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Memory Buffer Events (Onchain Readable Cache)  [LEGACY]
// ═══════════════════════════════════════════════════

/// Buffer page created.
#[cfg(feature = "legacy-memory")]
#[event]
pub struct BufferCreatedEvent {
    pub session: Pubkey,
    pub buffer: Pubkey,
    pub authority: Pubkey,
    pub page_index: u32,
    pub timestamp: i64,
}

/// Data appended to buffer.
#[cfg(feature = "legacy-memory")]
#[event]
pub struct BufferAppendedEvent {
    pub session: Pubkey,
    pub buffer: Pubkey,
    pub page_index: u32,
    pub chunk_size: u16,
    pub total_size: u16,
    pub num_entries: u16,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Memory Digest Events (Proof-of-Memory)  [LEGACY]
// ═══════════════════════════════════════════════════

/// Digest proof posted (hash only, no data).
#[cfg(feature = "legacy-memory")]
#[event]
pub struct DigestPostedEvent {
    pub session: Pubkey,
    pub digest: Pubkey,
    pub content_hash: [u8; 32],
    pub data_size: u32,
    pub entry_index: u32,
    pub merkle_root: [u8; 32],
    pub timestamp: i64,
}

/// Data inscribed to TX log via digest. Permanent, zero rent.
/// Verify: sha256(data) == content_hash. Replay merkle chain → match root.
#[cfg(feature = "legacy-memory")]
#[event]
pub struct DigestInscribedEvent {
    pub session: Pubkey,
    pub digest: Pubkey,
    pub entry_index: u32,
    pub data: Vec<u8>,
    pub content_hash: [u8; 32],
    pub data_len: u32,
    pub merkle_root: [u8; 32],
    pub timestamp: i64,
}

/// Offchain storage pointer updated.
#[cfg(feature = "legacy-memory")]
#[event]
pub struct StorageRefUpdatedEvent {
    pub session: Pubkey,
    pub digest: Pubkey,
    pub storage_ref: [u8; 32],
    pub storage_type: u8,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════
//  Memory Ledger Events (Unified Onchain Memory)
// ═══════════════════════════════════════════════

/// Ledger write. Carries data in TX log — permanent, immutable.
#[event]
pub struct LedgerEntryEvent {
    pub session: Pubkey,
    pub ledger: Pubkey,
    pub entry_index: u32,
    pub data: Vec<u8>,
    pub content_hash: [u8; 32],
    pub data_len: u32,
    pub merkle_root: [u8; 32],
    pub timestamp: i64,
}

/// Ring sealed into permanent page. Write-once, no close exists.
#[event]
pub struct LedgerSealedEvent {
    pub session: Pubkey,
    pub ledger: Pubkey,
    pub page: Pubkey,
    pub page_index: u32,
    pub entries_in_page: u32,
    pub data_size: u32,
    pub merkle_root_at_seal: [u8; 32],
    pub timestamp: i64,
}
