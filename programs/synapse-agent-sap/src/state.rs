use anchor_lang::prelude::*;

// ═══════════════════════════════════════════════════════════════════
//  Enums
// ═══════════════════════════════════════════════════════════════════

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum TokenType {
    Sol,
    Usdc,
    Spl,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum PluginType {
    Memory,
    Validation,
    Delegation,
    Analytics,
    Governance,
    Custom,
}

impl PluginType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(PluginType::Memory),
            1 => Some(PluginType::Validation),
            2 => Some(PluginType::Delegation),
            3 => Some(PluginType::Analytics),
            4 => Some(PluginType::Governance),
            5 => Some(PluginType::Custom),
            _ => None,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum SettlementMode {
    /// Per-call onchain transfer
    Instant,
    /// Pre-funded escrow PDA, draw per call
    Escrow,
    /// Offchain accumulation, periodic settle
    Batched,
    /// HTTP x402 protocol (default)
    X402,
}

// ═══════════════════════════════════════════════════════════════════
//  Helper Structs
// ═══════════════════════════════════════════════════════════════════

/// Agent capability descriptor.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Capability {
    /// Capability ID, e.g. "jupiter:swap"
    #[max_len(64)]
    pub id: String,
    /// Description
    #[max_len(128)]
    pub description: Option<String>,
    /// Protocol group, e.g. "jupiter"
    #[max_len(64)]
    pub protocol_id: Option<String>,
    /// Semver, e.g. "1.0.0"
    #[max_len(16)]
    pub version: Option<String>,
}

/// Volume curve breakpoint for tiered billing.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct VolumeCurveBreakpoint {
    /// Cumulative calls threshold
    pub after_calls: u32,
    /// Price per call after threshold (smallest unit)
    pub price_per_call: u64,
}

/// Pricing tier for agent services.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct PricingTier {
    /// Tier ID, e.g. "standard"
    #[max_len(32)]
    pub tier_id: String,
    /// Base price per call (smallest unit)
    pub price_per_call: u64,
    /// Price floor (optional)
    pub min_price_per_call: Option<u64>,
    /// Price ceiling (optional)
    pub max_price_per_call: Option<u64>,
    /// Max calls/sec
    pub rate_limit: u32,
    /// Max calls/session (0=unlimited)
    pub max_calls_per_session: u32,
    /// Max burst/sec (optional)
    pub burst_limit: Option<u32>,
    /// Token type
    pub token_type: TokenType,
    /// SPL mint (required if Spl)
    pub token_mint: Option<Pubkey>,
    /// Token decimals (9=SOL, 6=USDC)
    pub token_decimals: Option<u8>,
    /// Settlement mode
    pub settlement_mode: Option<SettlementMode>,
    /// Min escrow deposit (Escrow mode)
    pub min_escrow_deposit: Option<u64>,
    /// Batch interval sec (Batched mode)
    pub batch_interval_sec: Option<u32>,
    /// Volume discount curve (max 5)
    #[max_len(5)]
    pub volume_curve: Option<Vec<VolumeCurveBreakpoint>>,
}

/// Reference to an active plugin PDA
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct PluginRef {
    pub plugin_type: PluginType,
    pub pda: Pubkey,
}

/// Individual settlement entry for batch.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Settlement {
    /// Calls to bill
    pub calls_to_settle: u64,
    /// sha256 proof of service
    pub service_hash: [u8; 32],
}

// ═══════════════════════════════════════════════════════════════════
//  Account: AgentAccount (Core Identity PDA)
//  Seeds: ["sap_agent", wallet_pubkey]
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct AgentAccount {
    // ── Fixed Fields ──
    pub bump: u8,
    pub version: u8,
    pub wallet: Pubkey,
    #[max_len(64)]
    pub name: String,
    #[max_len(256)]
    pub description: String,
    #[max_len(128)]
    pub agent_id: Option<String>,       // DID-style identifier
    #[max_len(256)]
    pub agent_uri: Option<String>,
    #[max_len(256)]
    pub x402_endpoint: Option<String>,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,

    // ── Reputation (computed onchain, NOT user-settable) ──
    pub reputation_score: u32,   // 0-10000 (2 decimal precision)
    pub total_feedbacks: u32,
    pub reputation_sum: u64,     // sum of active feedback scores (for incremental calc)
    /// DEPRECATED: use AgentStats.total_calls_served (hot-path PDA)
    pub total_calls_served: u64,
    pub avg_latency_ms: u32,    // average response latency (self-reported)
    pub uptime_percent: u8,     // 0-100 (self-reported)

    // ── Dynamic Fields ──
    #[max_len(10)]
    pub capabilities: Vec<Capability>,
    #[max_len(5)]
    pub pricing: Vec<PricingTier>,
    #[max_len(5, 64)]
    pub protocols: Vec<String>,
    #[max_len(5)]
    pub active_plugins: Vec<PluginRef>,
}

impl AgentAccount {
    pub const MAX_NAME_LEN: usize = 64;
    pub const MAX_DESC_LEN: usize = 256;
    pub const MAX_URI_LEN: usize = 256;
    pub const MAX_AGENT_ID_LEN: usize = 128;
    pub const MAX_CAPABILITIES: usize = 10;
    pub const MAX_PRICING_TIERS: usize = 5;
    pub const MAX_PROTOCOLS: usize = 5;
    pub const MAX_PLUGINS: usize = 5;
    pub const MAX_VOLUME_CURVE_POINTS: usize = 5;
    pub const VERSION: u8 = 1;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: FeedbackAccount (Trustless Reputation)
//  Seeds: ["sap_feedback", agent_pda, reviewer_pubkey]
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct FeedbackAccount {
    pub bump: u8,
    pub agent: Pubkey,       // the agent PDA this feedback targets
    pub reviewer: Pubkey,    // the wallet that left the feedback
    pub score: u16,          // 0-1000
    #[max_len(32)]
    pub tag: String,         // e.g. "quality", "speed", "reliability"
    pub comment_hash: Option<[u8; 32]>,  // SHA-256 of IPFS comment
    pub created_at: i64,
    pub updated_at: i64,
    pub is_revoked: bool,
}

impl FeedbackAccount {
    pub const MAX_TAG_LEN: usize = 32;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: CapabilityIndex (Scalable Discovery)
//  Seeds: ["sap_cap_idx", sha256(capability_id)]
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct CapabilityIndex {
    pub bump: u8,
    #[max_len(64)]
    pub capability_id: String,
    pub capability_hash: [u8; 32],
    #[max_len(100)]
    pub agents: Vec<Pubkey>,      // agent PDAs with this capability
    pub total_pages: u8,          // for overflow pagination
    pub last_updated: i64,
}

impl CapabilityIndex {
    pub const MAX_AGENTS: usize = 100;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: ProtocolIndex
//  Seeds: ["sap_proto_idx", sha256(protocol_id)]
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct ProtocolIndex {
    pub bump: u8,
    #[max_len(64)]
    pub protocol_id: String,
    pub protocol_hash: [u8; 32],
    #[max_len(100)]
    pub agents: Vec<Pubkey>,
    pub total_pages: u8,
    pub last_updated: i64,
}

impl ProtocolIndex {
    pub const MAX_AGENTS: usize = 100;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: GlobalRegistry (Network-wide Stats Singleton)
//  Seeds: ["sap_global"]
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct GlobalRegistry {
    pub bump: u8,
    pub total_agents: u64,
    pub active_agents: u64,
    pub total_feedbacks: u64,
    pub total_capabilities: u32,
    pub total_protocols: u32,
    pub last_registered_at: i64,
    pub initialized_at: i64,
    pub authority: Pubkey,
    pub total_tools: u32,
    pub total_vaults: u32,
    /// DEPRECATED: escrow no longer updates GlobalRegistry
    pub total_escrows: u32,
    pub total_attestations: u32,
}

// ═══════════════════════════════════════════════════════════════════
//  Account: PluginSlot (Extensible PDA)  [LEGACY — gated behind "legacy-memory"]
//  Seeds: ["sap_plugin", agent_pda, plugin_type_u8]
// ═══════════════════════════════════════════════════════════════════

#[cfg(feature = "legacy-memory")]
#[derive(InitSpace)]
#[account]
pub struct PluginSlot {
    pub bump: u8,
    pub agent: Pubkey,
    pub plugin_type: PluginType,
    pub is_active: bool,
    pub initialized_at: i64,
    pub last_updated: i64,
    pub data_account: Option<Pubkey>,
}

// ═══════════════════════════════════════════════════════════════════
//  Account: MemoryEntry (Hybrid IPFS + Onchain)  [LEGACY — gated behind "legacy-memory"]
//  Seeds: ["sap_memory", agent_pda, entry_hash]
// ═══════════════════════════════════════════════════════════════════

#[cfg(feature = "legacy-memory")]
#[derive(InitSpace)]
#[account]
pub struct MemoryEntry {
    pub bump: u8,
    pub agent: Pubkey,
    pub entry_hash: [u8; 32],
    #[max_len(32)]
    pub content_type: String,
    #[max_len(64)]
    pub ipfs_cid: Option<String>,
    pub total_chunks: u8,
    pub total_size: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[cfg(feature = "legacy-memory")]
impl MemoryEntry {
    pub const MAX_CONTENT_TYPE_LEN: usize = 32;
    pub const MAX_IPFS_CID_LEN: usize = 64;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: MemoryChunk (Onchain Data Chunk)  [LEGACY — gated behind "legacy-memory"]
//  Seeds: ["sap_mem_chunk", memory_entry_pda, chunk_index]
// ═══════════════════════════════════════════════════════════════════

#[cfg(feature = "legacy-memory")]
#[derive(InitSpace)]
#[account]
pub struct MemoryChunk {
    pub bump: u8,
    pub memory_entry: Pubkey,
    pub chunk_index: u8,
    #[max_len(900)]
    pub data: Vec<u8>,
}

#[cfg(feature = "legacy-memory")]
impl MemoryChunk {
    pub const MAX_CHUNK_SIZE: usize = 900;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: MemoryVault (Encrypted Inscription Vault)
//  Seeds: ["sap_vault", agent_pda]
//
//  Data is NOT stored in accounts — it's inscribed into transaction
//  logs via Anchor events (zero rent).  The vault holds only
//  encryption metadata + aggregate counters.
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct MemoryVault {
    pub bump: u8,
    pub agent: Pubkey,            // the AgentAccount PDA
    pub wallet: Pubkey,           // owner wallet (for auth chain)
    pub vault_nonce: [u8; 32],    // random salt for PBKDF2 key derivation (public)
    pub total_sessions: u32,
    pub total_inscriptions: u64,
    pub total_bytes_inscribed: u64,
    pub created_at: i64,
    pub protocol_version: u8,     // protocol version for event format compat (v1)
    pub nonce_version: u32,       // increments on each nonce rotation (0 = original)
    pub last_nonce_rotation: i64, // timestamp of last rotation (0 = never)
}

impl MemoryVault {
    pub const PROTOCOL_VERSION: u8 = 1;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: SessionLedger (Compact Session Index)
//  Seeds: ["sap_session", vault_pda, session_hash]
//
//  Tracks how many inscriptions were written to this session.
//  Actual data lives in TX logs.  Counters sized for long-lived
//  sessions (u32 sequence → 4B entries, u64 bytes → exabytes).
//
//  Inscriptions are grouped into epochs (default 1000 per epoch).
//  Each epoch has its own EpochPage PDA that acts as a scan target
//  for getSignaturesForAddress, enabling O(1) epoch-level queries.
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct SessionLedger {
    pub bump: u8,
    pub vault: Pubkey,            // reference to MemoryVault PDA
    pub session_hash: [u8; 32],   // SHA-256 of session identifier
    pub sequence_counter: u32,    // next expected sequence number (4B max)
    pub total_bytes: u64,         // cumulative encrypted bytes (u64 for GB+)
    pub current_epoch: u32,       // current epoch index
    pub total_epochs: u32,        // how many epochs have been created
    pub created_at: i64,
    pub last_inscribed_at: i64,
    pub is_closed: bool,
    /// Rolling merkle: sha256(prev_root || content_hash). Tamper-proof chain.
    pub merkle_root: [u8; 32],
    /// Checkpoints created for this session.
    pub total_checkpoints: u32,
    /// Last content_hash. O(1) change detection. [0;32] = none.
    pub tip_hash: [u8; 32],
}

impl SessionLedger {
    pub const MAX_INSCRIPTION_SIZE: usize = 750;
    pub const INSCRIPTIONS_PER_EPOCH: u32 = 1000;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: EpochPage (Per-Epoch Scan Target)
//  Seeds: ["sap_epoch", session_pda, epoch_index(u32 LE)]
//
//  Tiny PDA (~100 bytes) — one per epoch (every 1000 inscriptions).
//  The inscribe_memory TX references this PDA so that
//  getSignaturesForAddress(epochPagePDA) returns ONLY the TXs
//  in that epoch.  This gives O(1) random access to any range
//  of inscriptions.
//
//  Auto-created via init_if_needed on the first inscription of
//  each new epoch.
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct EpochPage {
    pub bump: u8,
    pub session: Pubkey,          // parent SessionLedger PDA
    pub epoch_index: u32,          // 0, 1, 2, ...
    pub start_sequence: u32,       // first sequence in this epoch
    pub inscription_count: u16,    // inscriptions written in this epoch
    pub total_bytes: u32,          // bytes inscribed in this epoch
    pub first_ts: i64,             // timestamp of first inscription
    pub last_ts: i64,              // timestamp of last inscription
}



// ═══════════════════════════════════════════════════════════════════
//  Account: VaultDelegate (Hot Wallet Authorization)
//  Seeds: ["sap_delegate", vault_pda, delegate_pubkey]
//
//  Allows a secondary wallet (hot wallet) to perform vault
//  operations on behalf of the owner.  Permissions are a bitmask:
//    bit 0 (1) = inscribe_memory
//    bit 1 (2) = close_session
//    bit 2 (4) = open_session
//  Optionally expires at a given unix timestamp.
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct VaultDelegate {
    pub bump: u8,
    pub vault: Pubkey,            // parent MemoryVault PDA
    pub delegate: Pubkey,         // authorized hot wallet pubkey
    pub permissions: u8,          // bitmask of allowed operations
    pub expires_at: i64,          // 0 = never expires
    pub created_at: i64,
}

impl VaultDelegate {
    pub const PERMISSION_INSCRIBE: u8      = 1;
    pub const PERMISSION_CLOSE_SESSION: u8 = 2;
    pub const PERMISSION_OPEN_SESSION: u8  = 4;
    pub const ALL_PERMISSIONS: u8          = 7;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: ToolDescriptor (Onchain Tool Schema Registry)
//  Seeds: ["sap_tool", agent_pda, tool_name_hash]
//
//  Each tool an agent exposes gets a ToolDescriptor PDA.  It holds
//  compact metadata + hashes of the full schemas.  The full JSON
//  schemas are inscribed in TX logs via ToolSchemaPublishedEvent
//  (zero rent, immutable, verifiable against the onchain hash).
//
//  Designed for x402 + AI agent discovery:
//    - Any client can enumerate an agent's tools
//    - Schema hashes verify that decoded data matches the spec
//    - Version chain enables historical schema lookup
//    - category + protocol_hash enable cross-agent tool search
// ═══════════════════════════════════════════════════════════════════

/// HTTP method enum for tool descriptor.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum ToolHttpMethod {
    Get,
    Post,
    Put,
    Delete,
    /// Compound tool — chains multiple HTTP calls
    Compound,
}

impl ToolHttpMethod {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(ToolHttpMethod::Get),
            1 => Some(ToolHttpMethod::Post),
            2 => Some(ToolHttpMethod::Put),
            3 => Some(ToolHttpMethod::Delete),
            4 => Some(ToolHttpMethod::Compound),
            _ => None,
        }
    }
}

/// Tool category for discovery filtering.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum ToolCategory {
    Swap,        // 0 — token swaps
    Lend,        // 1 — lending/borrowing
    Stake,       // 2 — staking/validator
    Nft,         // 3 — NFT mint/trade
    Payment,     // 4 — payments/transfers
    Data,        // 5 — data queries/feeds
    Governance,  // 6 — DAO/voting
    Bridge,      // 7 — cross-chain
    Analytics,   // 8 — onchain analytics
    Custom,      // 9 — uncategorized
}

impl ToolCategory {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(ToolCategory::Swap),
            1 => Some(ToolCategory::Lend),
            2 => Some(ToolCategory::Stake),
            3 => Some(ToolCategory::Nft),
            4 => Some(ToolCategory::Payment),
            5 => Some(ToolCategory::Data),
            6 => Some(ToolCategory::Governance),
            7 => Some(ToolCategory::Bridge),
            8 => Some(ToolCategory::Analytics),
            9 => Some(ToolCategory::Custom),
            _ => None,
        }
    }
}

#[derive(InitSpace)]
#[account]
pub struct ToolDescriptor {
    pub bump: u8,
    pub agent: Pubkey,                 // parent AgentAccount PDA
    pub tool_name_hash: [u8; 32],      // sha256(tool_name) — used in PDA seed
    #[max_len(32)]
    pub tool_name: String,             // e.g. "getQuote", "smartSwap" (max 32)
    pub protocol_hash: [u8; 32],       // sha256(protocol_id) — links to ProtocolIndex
    pub version: u16,                  // schema version (1, 2, 3, ...)
    pub description_hash: [u8; 32],    // sha256 of full tool description
    pub input_schema_hash: [u8; 32],   // sha256 of input JSON schema
    pub output_schema_hash: [u8; 32],  // sha256 of output JSON schema
    pub http_method: ToolHttpMethod,   // GET, POST, PUT, DELETE, Compound
    pub category: ToolCategory,        // Swap, Lend, Data, etc.
    pub params_count: u8,              // total input parameters
    pub required_params: u8,           // required input parameters
    pub is_compound: bool,             // chains multiple calls (e.g. smartSwap)
    pub is_active: bool,               // can be deactivated without closing
    pub total_invocations: u64,        // call counter for analytics
    pub created_at: i64,
    pub updated_at: i64,
    pub previous_version: Pubkey,      // Pubkey::default() if first version
}

impl ToolDescriptor {
    pub const MAX_TOOL_NAME_LEN: usize = 32;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: SessionCheckpoint (Fast-Sync Snapshot)
//  Seeds: ["sap_checkpoint", session_pda, checkpoint_index(u32 LE)]
//
//  Periodic snapshot of the session's merkle root + counters.
//  Enables:
//    - Fast sync: start from nearest checkpoint instead of genesis
//    - Parallel verification: verify epochs independently
//    - Recovery points: deterministic state at any checkpoint
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct SessionCheckpoint {
    pub bump: u8,
    pub session: Pubkey,            // parent SessionLedger PDA
    pub checkpoint_index: u32,      // 0, 1, 2, ...
    pub merkle_root: [u8; 32],      // merkle accumulator root at this point
    pub sequence_at: u32,           // sequence_counter at checkpoint
    pub epoch_at: u32,              // current_epoch at checkpoint
    pub total_bytes_at: u64,        // cumulative bytes at checkpoint
    pub inscriptions_at: u64,       // total inscriptions up to this point
    pub created_at: i64,
}



// ═══════════════════════════════════════════════════════════════════
//  Account: EscrowAccount (x402 Pre-Funded Micropayments)
//  Seeds: ["sap_escrow", agent_pda, depositor_wallet]
//
//  Enables trustless micropayments between clients and agents.
//  The client pre-funds the escrow at a locked-in price per call.
//  The agent settles onchain after serving calls, emitting
//  a PaymentSettledEvent with service_hash as proof of work.
//
//  x402 flow:
//    1. Client discovers agent pricing → deposits into escrow
//    2. Client calls agent via x402 HTTP endpoint
//    3. Agent settles onchain (claims payment, receipt in TX log)
//    4. Client can withdraw unused balance at any time
//    5. Close escrow when done (rent returned to depositor)
//
//  Onchain guarantees:
//    - Price per call is immutable (agent can't change it)
//    - max_calls limits total exposure
//    - Client can always withdraw remaining balance
//    - PaymentSettledEvent = permanent zero-rent receipt
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct EscrowAccount {
    pub bump: u8,
    pub agent: Pubkey,            // provider agent PDA
    pub depositor: Pubkey,        // client wallet that funded the escrow
    pub agent_wallet: Pubkey,     // agent owner wallet (settlement destination)
    pub balance: u64,             // available balance (lamports or smallest token unit)
    pub total_deposited: u64,     // lifetime deposits
    pub total_settled: u64,       // lifetime settlements
    pub total_calls_settled: u64, // lifetime calls settled
    pub price_per_call: u64,      // base price per call (smallest unit)
    pub max_calls: u64,           // max calls allowed (0 = unlimited)
    pub created_at: i64,
    pub last_settled_at: i64,
    pub expires_at: i64,          // 0 = never expires
    // ── Native Solana Enhancements ──
    /// Tiered pricing curve (max 5 breakpoints). Spans tier boundaries.
    #[max_len(5)]
    pub volume_curve: Vec<VolumeCurveBreakpoint>,
    /// None = SOL, Some = SPL token.
    pub token_mint: Option<Pubkey>,
    /// Token decimals (9=SOL, 6=USDC). Informational.
    pub token_decimals: u8,
}

impl EscrowAccount {
    pub const MAX_VOLUME_CURVE: usize = 5;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: AgentStats (Lightweight Hot-Path Metrics)
//  Seeds: ["sap_stats", agent_pda]
//
//  Extracted from AgentAccount to avoid deserializing 8 KB
//  on every settle_calls / report_calls.  The hot settlement
//  path loads only this 106-byte PDA instead of the full
//  AgentAccount — ~76× less data per TX.
//
//  Created alongside AgentAccount during register_agent.
//  Updated by: settle_calls, settle_batch, report_calls,
//              deactivate_agent, reactivate_agent.
//  Closed alongside AgentAccount during close_agent.
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct AgentStats {
    pub bump: u8,
    pub agent: Pubkey,            // the AgentAccount PDA
    pub wallet: Pubkey,           // owner wallet (for auth chain)
    pub total_calls_served: u64,  // authoritative call counter
    pub is_active: bool,          // mirrored from AgentAccount
    pub updated_at: i64,
}

// ═══════════════════════════════════════════════════════════════════
//  Account: ToolCategoryIndex (Cross-Agent Tool Discovery)
//  Seeds: ["sap_tool_cat", category_u8]
//
//  Lists all ToolDescriptor PDAs across ALL agents for a given
//  category (Swap, Lend, Data, etc.).  Enables queries like:
//    "Show me all Swap tools" → fetch ToolCategoryIndex(category=0)
//    "Show me all Data feed tools" → fetch ToolCategoryIndex(category=5)
//  Then fetch each ToolDescriptor for details + schema hashes.
//
//  Essential for agent-to-agent composition:
//    Agent A needs a Swap tool → queries ToolCategoryIndex(Swap)
//    → discovers Agent B's getQuote tool → calls via x402 + escrow
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct ToolCategoryIndex {
    pub bump: u8,
    pub category: u8,             // ToolCategory enum value (0-9)
    #[max_len(100)]
    pub tools: Vec<Pubkey>,       // ToolDescriptor PDAs
    pub total_pages: u8,          // for overflow pagination
    pub last_updated: i64,
}

impl ToolCategoryIndex {
    pub const MAX_TOOLS: usize = 100;
}

// ═══════════════════════════════════════════════════════════════════
//  Account: AgentAttestation (Web of Trust)
//  Seeds: ["sap_attest", agent_pda, attester_wallet]
//
//  Third-party verifiable trust signal — an authority vouches for
//  an agent.  Unlike feedback (user reviews, score-based),
//  attestations are institutional (boolean + type).
//
//  Use cases:
//    - "API verified by Jupiter" (type: "api_verified")
//    - "Code audited by OtterSec" (type: "audited")
//    - "Official Solana partner" (type: "partner")
//    - "Data feed verified by Chainlink" (type: "data_verified")
//
//  Anyone can attest for any agent (one per pair).
//  Trust comes from WHO is attesting, not the attestation itself.
//  Clients filter by attester pubkey to find attestations from
//  trusted authorities.
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct AgentAttestation {
    pub bump: u8,
    pub agent: Pubkey,             // the agent PDA
    pub attester: Pubkey,          // authority wallet that vouches
    #[max_len(32)]
    pub attestation_type: String,  // e.g. "verified", "audited", "partner"
    pub metadata_hash: [u8; 32],   // sha256 of attestation evidence (offchain)
    pub is_active: bool,
    pub expires_at: i64,           // 0 = never expires
    pub created_at: i64,
    pub updated_at: i64,
}

impl AgentAttestation {
    pub const MAX_TYPE_LEN: usize = 32;
}

// ═══════════════════════════════════════════════════════════════════════
//  Account: MemoryBuffer (Onchain Readable Session Cache)
//  Seeds: ["sap_buffer", session_pda, page_index(u32 LE)]
//
//  Complementary to TX log inscriptions.  While inscriptions
//  are permanent + zero rent but require archival RPC access,
//  MemoryBuffers store data directly in PDA accounts — readable
//  via plain getAccountInfo() on ANY free RPC.
//
//  Uses Anchor realloc: PDA starts tiny (~101 bytes, ≈0.001 SOL),
//  grows dynamically as data is appended.  Developer pays rent
//  ONLY for the bytes actually stored.  ALL rent is reclaimable
//  via close_buffer.
//
//  Cost model (proportional):
//    Empty      : 101 bytes  →  ≈0.001 SOL
//    500 bytes  : 601 bytes  →  ≈0.005 SOL
//    2.5 KB     : 2601 bytes →  ≈0.021 SOL
//    10 KB (max): 10101 bytes → ≈0.073 SOL
//    close_buffer → reclaim ALL rent
//
//  Developer workflow:
//    1. create_buffer  → tiny PDA (~0.001 SOL)
//    2. append_buffer  → realloc + append (≤750B per call)
//    3. Read anytime   → getAccountInfo(bufferPDA) — free!
//    4. close_buffer   → reclaim all rent
//
//  Choice matrix:
//    inscribe_memory  → permanent, zero rent, needs archival RPC
//    compact_inscribe → permanent, zero rent, simpler API
//    write to buffer  → ephemeral, tiny rent, readable on ANY RPC
// ═══════════════════════════════════════════════════════════════════════

#[cfg(feature = "legacy-memory")]
#[derive(InitSpace)]
#[account]
pub struct MemoryBuffer {
    pub bump: u8,
    pub session: Pubkey,       // parent SessionLedger PDA
    pub authority: Pubkey,     // wallet authorized to write/close
    pub page_index: u32,       // buffer page number (0, 1, 2, ...)
    pub num_entries: u16,      // how many append_buffer calls
    pub total_size: u16,       // current total data bytes
    pub created_at: i64,
    pub updated_at: i64,
    #[max_len(10000)]
    pub data: Vec<u8>,         // appended data (grows via realloc)
}

#[cfg(feature = "legacy-memory")]
impl MemoryBuffer {
    /// Max bytes per single append_buffer call (same as inscriptions)
    pub const MAX_WRITE_SIZE: usize = 750;
    /// Max total data bytes in one buffer page
    pub const MAX_TOTAL_SIZE: usize = 10_000;

    /// Header space: everything except the Vec's data content.
    /// Used for init (create_buffer) and realloc (append_buffer).
    ///   realloc_target = HEADER_SPACE + total_data_bytes
    pub const HEADER_SPACE: usize = 8   // discriminator
        + 1    // bump
        + 32   // session
        + 32   // authority
        + 4    // page_index
        + 2    // num_entries
        + 2    // total_size
        + 8    // created_at
        + 8    // updated_at
        + 4;   // vec length prefix
    // Note: NO padding — space is allocated dynamically via realloc.
    // HEADER_SPACE = 101 bytes
}

// ═══════════════════════════════════════════════════════════════════════
//  Account: MemoryDigest — Proof-of-Memory Protocol
//  Seeds: ["sap_digest", session_pda]
//
//  PARADIGM SHIFT: Don't store data onchain — store PROOF
//  that the data exists.  Actual data lives offchain (IPFS,
//  Arweave, Shadow Drive, S3, local DB) at a fraction of the cost.
//
//  Onchain: a FIXED-SIZE PDA (~230 bytes, ≈0.002 SOL) that NEVER
//  grows.  Contains a rolling merkle root, latest content hash,
//  and a pointer to offchain storage.
//
//  Each post_digest call costs ONLY the Solana TX fee (~0.000005
//  SOL) — zero additional rent because the PDA never grows.
//
//  Cost model (vs alternatives):
//    ┌─────────────────┬─────────────────┬─────────────────────┐
//    │ 10K entries 1KB │ MemoryDigest    │ Onchain storage     │
//    ├─────────────────┼─────────────────┼─────────────────────┤
//    │ Onchain rent   │ 0.002 SOL fixed │ ~69.6 SOL           │
//    │ TX fees         │ 0.05 SOL        │ 0.05 SOL            │
//    │ Offchain data  │ ~$0.02 IPFS     │ n/a                 │
//    │ TOTAL           │ ≈0.052 SOL      │ ≈69.65 SOL          │
//    └─────────────────┴─────────────────┴─────────────────────┘
//
//  Verification: Any third party can verify data integrity:
//    1. Fetch data from offchain storage
//    2. sha256(data) → must match content_hash in TX log event
//    3. Replay merkle chain → must match onchain merkle_root
//
//  Storage types:
//    0 = None       (no offchain storage pointer set yet)
//    1 = IPFS       (storage_ref = sha256 of CID)
//    2 = Arweave    (storage_ref = 32-byte TX ID)
//    3 = ShadowDrive(storage_ref = sha256 of URL)
//    4 = HTTP/S     (storage_ref = sha256 of URL)
//    5 = Filecoin   (storage_ref = deal CID hash)
//    6-255 = Custom
// ═══════════════════════════════════════════════════════════════════════

#[cfg(feature = "legacy-memory")]
#[derive(InitSpace)]
#[account]
pub struct MemoryDigest {
    pub bump: u8,
    pub session: Pubkey,        // parent SessionLedger PDA
    pub authority: Pubkey,      // wallet authorized to post/close
    pub num_entries: u32,       // total post_digest calls
    pub merkle_root: [u8; 32],  // rolling hash: sha256(prev_root || content_hash)
    pub latest_hash: [u8; 32],  // most recent content_hash
    pub total_data_size: u64,   // cumulative bytes (tracked, NOT stored onchain)
    pub storage_ref: [u8; 32],  // pointer to offchain bundle (type-dependent)
    pub storage_type: u8,       // see storage types above
    pub created_at: i64,
    pub updated_at: i64,
}



// ═══════════════════════════════════════════════════════════════════════
//  Account: MemoryLedger — Unified Onchain Memory
//  Seeds: ["sap_ledger", session_pda]
//
//  THE RECOMMENDED MEMORY SYSTEM.  Combines the best of
//  Digest (permanent TX logs + merkle proof) and Buffer
//  (instant readability) into ONE unified fixed-cost PDA.
//
//  HOW IT WORKS:
//    - A 4KB sliding-window ring buffer lives inside the PDA.
//      Latest entries are always readable via getAccountInfo()
//      on ANY free RPC — no archival access needed.
//    - EVERY write is also emitted as a TX log event.
//      The full history is permanent, immutable, onchain.
//    - A rolling merkle root proves the integrity of all data.
//
//  TWO READ PATHS:
//    Hot path  → getAccountInfo(ledgerPDA) → latest ~10-20 msgs → FREE
//    Cold path → getSignaturesForAddress + getTransaction    → full history
//
//  COST MODEL:
//    Init         : ~0.032 SOL (FIXED, reclaimable, PDA never grows)
//    Per write    : ~0.000005 SOL (TX fee only, zero additional rent)
//    1K writes    : ~0.037 SOL total (init + TX fees)
//    10K writes   : ~0.082 SOL total
//    close_ledger : reclaim ~0.032 SOL
//
//  RING BUFFER FORMAT (inside PDA):
//    Each entry: [data_len: u16 LE][data: u8 * data_len]
//    On write: if entry doesn't fit, oldest entries are drained
//    from the front until there's room.  Evicted entries remain
//    permanently in TX logs.
//
//  WHY THIS REPLACES THE OTHER 3 SYSTEMS:
//    vs Vault  : simpler DX (2 args vs 8), no epoch pages needed
//    vs Buffer : fixed cost (never grows), permanent TX log backup
//    vs Digest : instant readability via ring buffer in PDA
// ═══════════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct MemoryLedger {
    pub bump: u8,
    pub session: Pubkey,          // parent SessionLedger PDA
    pub authority: Pubkey,        // wallet authorized to write/close
    pub num_entries: u32,         // total writes (ever, including evicted)
    pub merkle_root: [u8; 32],    // rolling hash of ALL entries
    pub latest_hash: [u8; 32],    // most recent content_hash
    pub total_data_size: u64,     // cumulative bytes written (ever)
    pub created_at: i64,
    pub updated_at: i64,
    pub num_pages: u32,           // sealed archive pages (permanent, immutable)
    #[max_len(4096)]
    pub ring: Vec<u8>,            // sliding-window ring buffer
}

impl MemoryLedger {
    /// Ring buffer capacity. ~5-20 entries depending on msg size.
    pub const RING_CAPACITY: usize = 4096;
}

// ═══════════════════════════════════════════════════════════════════════
//  Account: LedgerPage — Sealed Archive (Write-Once, Never-Delete)
//  Seeds: ["sap_page", ledger_pda, page_index_u32_le]
//
//  Created by seal_ledger().  Contains a frozen snapshot of the ring
//  buffer at the time of sealing.  NO close instruction exists —
//  pages are PERMANENTLY and IRREVOCABLY onchain.
//
//  Even the authority cannot delete them.  The program owns the PDA
//  and provides no instruction to close it.  If the program is made
//  non-upgradeable, pages become truly immutable forever.
//
//  Pages are discoverable by sequential index:
//    ["sap_page", ledger_pda, 0_u32_le] → first page
//    ["sap_page", ledger_pda, 1_u32_le] → second page
//    ...
//
//  Cost: ~0.031 SOL per page (permanent, NOT reclaimable by design).
//  This is the PRICE OF IMMUTABILITY — you pay once, data lives forever.
// ═══════════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct LedgerPage {
    pub bump: u8,
    pub ledger: Pubkey,                // parent MemoryLedger PDA
    pub page_index: u32,               // sequential page number (0, 1, 2, ...)
    pub sealed_at: i64,                // unix timestamp when sealed
    pub entries_in_page: u32,          // number of entries in this page
    pub data_size: u32,                // raw data size in bytes
    pub merkle_root_at_seal: [u8; 32], // merkle root snapshot at time of seal
    #[max_len(4096)]
    pub data: Vec<u8>,                 // frozen ring buffer contents (IMMUTABLE)
}


