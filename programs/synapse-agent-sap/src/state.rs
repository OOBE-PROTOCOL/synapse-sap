use anchor_lang::prelude::*;

// ═══════════════════════════════════════════════════════════════════
//  v0.10 Hardening — Payment token allowlist
//
//  Only native SOL (token_mint = None) or USDC (mainnet/devnet mint)
//  are accepted as escrow payment tokens.  Any other SPL mint is
//  rejected at create_escrow time.  Existing escrows pre-v0.10 keep
//  working (settle/withdraw/close still functional) — the restriction
//  applies only to NEW escrows.
// ═══════════════════════════════════════════════════════════════════

/// USDC on Solana mainnet-beta (Circle).
pub const USDC_MAINNET: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
/// USDC on Solana devnet (Circle test mint).
pub const USDC_DEVNET: Pubkey = pubkey!("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");

/// Returns true if `mint` is an accepted payment token mint (USDC mainnet or devnet).
/// SOL acceptance is signalled by `token_mint = None` and is checked at the call site.
#[inline]
pub fn is_accepted_usdc_mint(mint: &Pubkey) -> bool {
    mint == &USDC_MAINNET || mint == &USDC_DEVNET
}

// ═══════════════════════════════════════════════════════════════════
//  Enums
// ═══════════════════════════════════════════════════════════════════

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum TokenType {
    Sol,
    Usdc,
    Spl,
}

/// Settlement security level — determines how settle_calls is authorized.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum SettlementSecurity {
    /// Legacy discriminant kept only for deserialization compatibility.
    SelfReport,
    /// Client co-signs every settlement TX
    CoSigned,
    /// Settlement enters pending state, dispute window applies
    DisputeWindow,
}

/// Dispute resolution outcome.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum DisputeOutcome {
    /// Pending — not yet resolved
    Pending,
    /// Resolved in favor of depositor (refund)
    DepositorWins,
    /// Resolved in favor of agent (release funds)
    AgentWins,
    /// Expired — no dispute filed, funds auto-released
    AutoReleased,
    /// Partial refund — some calls proven, some not
    PartialRefund,
    /// Split — irresolvable quality dispute, 50/50
    Split,
}

/// Dispute type — determines resolution path.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum DisputeType {
    /// Agent claims N calls but client received fewer → auto-resolvable via receipts
    NonDelivery,
    /// Agent delivered fewer calls than claimed → auto-resolvable via receipts
    PartialDelivery,
    /// Agent overcharged relative to agreed price → auto-resolvable via receipts
    Overcharge,
    /// Response quality is poor — requires bond, may escalate
    Quality,
}

/// Resolution layer — how the dispute was resolved.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum ResolutionLayer {
    /// Pending resolution
    Pending,
    /// Resolved automatically via receipt proofs
    Auto,
    /// Resolved via governance / timeout fallback
    Governance,
}

/// Subscription billing interval.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum BillingInterval {
    Daily,
    Weekly,
    Monthly,
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
    pub agent_id: Option<String>, // DID-style identifier
    #[max_len(256)]
    pub agent_uri: Option<String>,
    #[max_len(256)]
    pub x402_endpoint: Option<String>,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,

    // ── Reputation (computed onchain, NOT user-settable) ──
    pub reputation_score: u32, // 0-10000 (2 decimal precision)
    pub total_feedbacks: u32,
    pub reputation_sum: u64, // sum of active feedback scores (for incremental calc)
    /// DEPRECATED: use AgentStats.total_calls_served (hot-path PDA)
    pub total_calls_served: u64,
    pub avg_latency_ms: u32, // average response latency (self-reported)
    pub uptime_percent: u8,  // 0-100 (self-reported)

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
    pub agent: Pubkey,    // the agent PDA this feedback targets
    pub reviewer: Pubkey, // the wallet that left the feedback
    pub score: u16,       // 0-1000
    #[max_len(32)]
    pub tag: String, // e.g. "quality", "speed", "reliability"
    pub comment_hash: Option<[u8; 32]>, // SHA-256 of IPFS comment
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
    pub agents: Vec<Pubkey>, // agent PDAs with this capability
    pub total_pages: u8, // for overflow pagination
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
    pub agent: Pubkey,         // the AgentAccount PDA
    pub wallet: Pubkey,        // owner wallet (for auth chain)
    pub vault_nonce: [u8; 32], // random salt for PBKDF2 key derivation (public)
    pub total_sessions: u32,
    pub total_inscriptions: u64,
    pub total_bytes_inscribed: u64,
    pub created_at: i64,
    pub protocol_version: u8, // protocol version for event format compat (v1)
    pub nonce_version: u32,   // increments on each nonce rotation (0 = original)
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
    pub vault: Pubkey,          // reference to MemoryVault PDA
    pub session_hash: [u8; 32], // SHA-256 of session identifier
    pub sequence_counter: u32,  // next expected sequence number (4B max)
    pub total_bytes: u64,       // cumulative encrypted bytes (u64 for GB+)
    pub current_epoch: u32,     // current epoch index
    pub total_epochs: u32,      // how many epochs have been created
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
    pub session: Pubkey,        // parent SessionLedger PDA
    pub epoch_index: u32,       // 0, 1, 2, ...
    pub start_sequence: u32,    // first sequence in this epoch
    pub inscription_count: u16, // inscriptions written in this epoch
    pub total_bytes: u32,       // bytes inscribed in this epoch
    pub first_ts: i64,          // timestamp of first inscription
    pub last_ts: i64,           // timestamp of last inscription
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
    pub vault: Pubkey,    // parent MemoryVault PDA
    pub delegate: Pubkey, // authorized hot wallet pubkey
    pub permissions: u8,  // bitmask of allowed operations
    pub expires_at: i64,  // 0 = never expires
    pub created_at: i64,
}

impl VaultDelegate {
    pub const PERMISSION_INSCRIBE: u8 = 1;
    pub const PERMISSION_CLOSE_SESSION: u8 = 2;
    pub const PERMISSION_OPEN_SESSION: u8 = 4;
    pub const ALL_PERMISSIONS: u8 = 7;
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
    Swap,       // 0 — token swaps
    Lend,       // 1 — lending/borrowing
    Stake,      // 2 — staking/validator
    Nft,        // 3 — NFT mint/trade
    Payment,    // 4 — payments/transfers
    Data,       // 5 — data queries/feeds
    Governance, // 6 — DAO/voting
    Bridge,     // 7 — cross-chain
    Analytics,  // 8 — onchain analytics
    Custom,     // 9 — uncategorized
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
    pub agent: Pubkey,            // parent AgentAccount PDA
    pub tool_name_hash: [u8; 32], // sha256(tool_name) — used in PDA seed
    #[max_len(32)]
    pub tool_name: String, // e.g. "getQuote", "smartSwap" (max 32)
    pub protocol_hash: [u8; 32],  // sha256(protocol_id) — links to ProtocolIndex
    pub version: u16,             // schema version (1, 2, 3, ...)
    pub description_hash: [u8; 32], // sha256 of full tool description
    pub input_schema_hash: [u8; 32], // sha256 of input JSON schema
    pub output_schema_hash: [u8; 32], // sha256 of output JSON schema
    pub http_method: ToolHttpMethod, // GET, POST, PUT, DELETE, Compound
    pub category: ToolCategory,   // Swap, Lend, Data, etc.
    pub params_count: u8,         // total input parameters
    pub required_params: u8,      // required input parameters
    pub is_compound: bool,        // chains multiple calls (e.g. smartSwap)
    pub is_active: bool,          // can be deactivated without closing
    pub total_invocations: u64,   // call counter for analytics
    pub created_at: i64,
    pub updated_at: i64,
    pub previous_version: Pubkey, // Pubkey::default() if first version
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
    pub session: Pubkey,       // parent SessionLedger PDA
    pub checkpoint_index: u32, // 0, 1, 2, ...
    pub merkle_root: [u8; 32], // merkle accumulator root at this point
    pub sequence_at: u32,      // sequence_counter at checkpoint
    pub epoch_at: u32,         // current_epoch at checkpoint
    pub total_bytes_at: u64,   // cumulative bytes at checkpoint
    pub inscriptions_at: u64,  // total inscriptions up to this point
    pub created_at: i64,
}

// ═══════════════════════════════════════════════════════════════════
//  Account: AgentPricingMenu (On-Chain Pricing Validation)
//  Seeds: ["sap_pricing", agent_pda]
//
//  Stores theagent's pricing tiers in a dedicated PDA so that
//  escrow_v2::create_escrow can validate `price_per_call` against
//  the published menu — preventing agents from changing prices
//  unilaterally after a client has locked funds.
//
//  Created during register_agent, updated by update_pricing_menu.
//  Must contain at least one tier.  USDC-only for commercial escrows.
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct AgentPricingMenu {
    pub bump: u8,
    pub agent: Pubkey, // the AgentAccount PDA
    /// Copy of AgentAccount pricing at creation time.
    /// Max 10 tiers — enough for typical SaaS ladder.
    #[max_len(10)]
    pub tiers: Vec<PricingTier>,
    pub updated_at: i64,
}

impl AgentPricingMenu {
    /// Returns true if a published tier matches the payment rail and price.
    pub fn validate_price(&self, token_mint: &Option<Pubkey>, price_per_call: u64) -> bool {
        self.tiers.iter().any(|t| {
            if t.price_per_call != price_per_call {
                return false;
            }

            match (token_mint, t.token_type) {
                (None, TokenType::Sol) => true,
                (Some(mint), TokenType::Usdc) => is_accepted_usdc_mint(mint),
                (Some(mint), TokenType::Spl) => t.token_mint == Some(*mint),
                _ => false,
            }
        })
    }
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
    pub agent: Pubkey,           // the AgentAccount PDA
    pub wallet: Pubkey,          // owner wallet (for auth chain)
    pub total_calls_served: u64, // authoritative call counter
    pub is_active: bool,         // mirrored from AgentAccount
    /// v0.12 H-1 hardening: counter of open escrows for this agent.
    /// Incremented on create_escrow, decremented on close_escrow.
    /// close_agent refuses to execute unless this is zero.
    pub active_escrows: u32,
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
    pub category: u8, // ToolCategory enum value (0-9)
    #[max_len(100)]
    pub tools: Vec<Pubkey>, // ToolDescriptor PDAs
    pub total_pages: u8, // for overflow pagination
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
    pub agent: Pubkey,    // the agent PDA
    pub attester: Pubkey, // authority wallet that vouches
    #[max_len(32)]
    pub attestation_type: String, // e.g. "verified", "audited", "partner"
    pub metadata_hash: [u8; 32], // sha256 of attestation evidence (offchain)
    pub is_active: bool,
    pub expires_at: i64, // 0 = never expires
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
    pub session: Pubkey,   // parent SessionLedger PDA
    pub authority: Pubkey, // wallet authorized to write/close
    pub page_index: u32,   // buffer page number (0, 1, 2, ...)
    pub num_entries: u16,  // how many append_buffer calls
    pub total_size: u16,   // current total data bytes
    pub created_at: i64,
    pub updated_at: i64,
    #[max_len(10000)]
    pub data: Vec<u8>, // appended data (grows via realloc)
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
        + 4; // vec length prefix
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
    pub session: Pubkey,       // parent SessionLedger PDA
    pub authority: Pubkey,     // wallet authorized to post/close
    pub num_entries: u32,      // total post_digest calls
    pub merkle_root: [u8; 32], // rolling hash: sha256(prev_root || content_hash)
    pub latest_hash: [u8; 32], // most recent content_hash
    pub total_data_size: u64,  // cumulative bytes (tracked, NOT stored onchain)
    pub storage_ref: [u8; 32], // pointer to offchain bundle (type-dependent)
    pub storage_type: u8,      // see storage types above
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
    pub session: Pubkey,       // parent SessionLedger PDA
    pub authority: Pubkey,     // wallet authorized to write/close
    pub num_entries: u32,      // total writes (ever, including evicted)
    pub merkle_root: [u8; 32], // rolling hash of ALL entries
    pub latest_hash: [u8; 32], // most recent content_hash
    pub total_data_size: u64,  // cumulative bytes written (ever)
    pub created_at: i64,
    pub updated_at: i64,
    pub num_pages: u32, // sealed archive pages (permanent, immutable)
    #[max_len(4096)]
    pub ring: Vec<u8>, // sliding-window ring buffer
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
    pub data: Vec<u8>, // frozen ring buffer contents (IMMUTABLE)
}

// ═══════════════════════════════════════════════════════════════════
//  SAP v2.1 — Protocol Upgrade Accounts
//
//  All new accounts are ADDITIVE — no existing state is modified.
//  Existing v1 instructions remain 100% backwards-compatible.
//  New functionality lives in new instruction modules.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  EscrowAccountV2 — Extended Escrow with Settlement Security
//  Seeds: ["sap_escrow_v2", agent_pda, depositor_wallet, nonce(u64 LE)]
//
//  1. nonce allows MULTIPLE escrows per (agent, depositor) pair
//  2. settlement_security selects one of 3 verification modes:
//     - SelfReport: backwards-compatible, agent settles unilaterally
//     - CoSigned:   BOTH agent + client sign every settlement TX
//     - DisputeWindow: settlement enters pending state for N slots
//  3. Built-in arbiter for dispute resolution
//  4. co_signer for bilateral settlement model
// ─────────────────────────────────────────────────────────────────

#[derive(InitSpace)]
#[account]
pub struct EscrowAccountV2 {
    pub bump: u8,
    pub version: u8, // 2
    pub agent: Pubkey,
    pub depositor: Pubkey,
    pub agent_wallet: Pubkey,
    pub escrow_nonce: u64, // allows multiple escrows per pair
    pub balance: u64,
    pub total_deposited: u64,
    pub total_settled: u64,
    pub total_calls_settled: u64,
    pub price_per_call: u64,
    pub max_calls: u64,
    pub created_at: i64,
    pub last_settled_at: i64,
    pub expires_at: i64,
    #[max_len(5)]
    pub volume_curve: Vec<VolumeCurveBreakpoint>,
    pub token_mint: Option<Pubkey>,
    pub token_decimals: u8,
    // ── v2 settlement security ──
    pub settlement_security: SettlementSecurity,
    pub dispute_window_slots: u64, // slots before auto-release (DisputeWindow)
    pub settlement_index: u64,     // monotonic settlement counter
    pub co_signer: Option<Pubkey>, // required co-signer (CoSigned)
    /// DEPRECATED in v0.7 — arbiter removed, disputes auto-resolved via receipts
    pub arbiter: Option<Pubkey>,
    // ── v2 pending totals ──
    pub pending_amount: u64, // total lamports locked in pending settlements
    pub pending_calls: u64,  // total calls in pending settlements
    // ── v0.7 receipt tracking ──
    pub receipt_batch_count: u32, // counter for ReceiptBatch PDAs
    // ── v0.13 hardening ──
    /// Cumulative dispute bonds held in escrow (orphan protection).
    pub dispute_bond_total: u64,
    /// Maximum balance allowed (set at creation time based on stake coverage).
    pub max_obligation: u64,
    /// Number of non-finalized PendingSettlement PDAs for this escrow.
    pub pending_settlement_count: u32,
}

impl EscrowAccountV2 {
    pub const VERSION: u8 = 2;
    pub const MAX_VOLUME_CURVE: usize = 5;
    /// v0.13: Maximum calls that can be settled in a single TX (prevents CU exhaustion).
    pub const MAX_CALLS_PER_SETTLEMENT: u64 = 10_000;
    /// v0.13: Maximum receipt proofs allowed in a single submit_receipt_proof call.
    pub const MAX_RECEIPT_PROOFS: usize = 100;
    /// v0.13: Maximum depth of a single merkle proof (2^32 leaves).
    pub const MAX_MERKLE_DEPTH: usize = 32;
}

// ─────────────────────────────────────────────────────────────────
//  ReceiptBatch — Merkle Root of Off-Chain Call Receipts
//  Seeds: ["sap_receipt", escrow_v2_pda, batch_index(u32 LE)]
//
//  Agent periodically commits a merkle root of dual-signed
//  call receipts to prove service delivery.  Each receipt is:
//    { call_id, tool_id, input_hash, output_hash,
//      timestamp, client_sig, agent_sig }
//
//  During disputes, the agent presents individual receipts +
//  merkle proofs to cryptographically prove delivery.
//
//  Cost: ~0.002 SOL per batch (fixed PDA, never grows).
//  One batch per billing period is typical.
// ─────────────────────────────────────────────────────────────────

#[derive(InitSpace)]
#[account]
pub struct ReceiptBatch {
    pub bump: u8,
    pub escrow: Pubkey,        // parent EscrowAccountV2 PDA
    pub batch_index: u32,      // sequential per escrow (0, 1, 2, ...)
    pub merkle_root: [u8; 32], // sha256 merkle tree root of all receipts
    pub call_count: u32,       // number of receipts in this batch
    pub period_start: i64,     // first receipt timestamp in batch
    pub period_end: i64,       // last receipt timestamp in batch
    pub inscribed_at: i64,     // when committed on-chain
}

// ─────────────────────────────────────────────────────────────────
//  PendingSettlement — Dispute Window Escrow Lock
//  Seeds: ["sap_pending", escrow_v2_pda, settlement_index(u64 LE)]
//
//  Created by settle_calls_v2 when settlement_security == DisputeWindow.
//  Funds are held in the escrow until the dispute window passes.
//  After release_slot, anyone can call finalize_settlement to
//  transfer funds to the agent.  The depositor can file a dispute
//  before release_slot, which freezes the settlement for arbiter
//  resolution.
// ─────────────────────────────────────────────────────────────────

#[derive(InitSpace)]
#[account]
pub struct PendingSettlement {
    pub bump: u8,
    pub escrow: Pubkey,
    pub agent: Pubkey,
    pub agent_wallet: Pubkey,
    pub depositor: Pubkey,
    pub settlement_index: u64,
    pub calls_to_settle: u64,
    pub amount: u64, // lamports/tokens locked
    pub service_hash: [u8; 32],
    pub receipt_merkle_root: [u8; 32], // v0.7: links to ReceiptBatch merkle root
    pub created_at: i64,
    pub release_slot: u64, // slot after which auto-release allowed
    pub is_finalized: bool,
    pub is_disputed: bool, // true if a dispute has been filed
    pub outcome: DisputeOutcome,
}

// ─────────────────────────────────────────────────────────────────
//  DisputeRecord — On-Chain Dispute with Auto-Resolution
//  Seeds: ["sap_dispute", pending_settlement_pda]
//
//  Filed by depositor during the dispute window.
//  v0.7: No arbiter — resolution is automatic via receipt proofs.
//  DisputeType determines resolution path:
//    - NonDelivery/PartialDelivery/Overcharge → auto via receipt proofs
//    - Quality → auto checks first, then governance/timeout fallback
// ─────────────────────────────────────────────────────────────────

#[derive(InitSpace)]
#[account]
pub struct DisputeRecord {
    pub bump: u8,
    pub pending_settlement: Pubkey,
    pub escrow: Pubkey,
    pub depositor: Pubkey,
    pub agent: Pubkey,
    pub dispute_type: DisputeType,     // v0.7: typed disputes
    pub evidence_hash: [u8; 32],       // sha256 of depositor's offchain evidence
    pub agent_evidence_hash: [u8; 32], // sha256 of agent's counter-evidence
    pub outcome: DisputeOutcome,
    pub resolution_layer: ResolutionLayer, // v0.7: how it was resolved
    pub created_at: i64,
    pub resolved_at: i64,          // 0 = unresolved
    pub resolution_hash: [u8; 32], // sha256 of resolution evidence
    pub slash_amount: u64,         // slashed from agent stake (if depositor wins)
    pub dispute_bond: u64,         // v0.7: 10% bond for Quality disputes
    pub proven_calls: u32,         // v0.7: calls proven via receipt proof
    pub claimed_calls: u32,        // v0.7: calls claimed in settlement
    pub proof_deadline: i64,       // v0.7: unix timestamp — agent must prove by this
}

// ─────────────────────────────────────────────────────────────────
//  AgentStake — Collateral for Honest Behavior
//  Seeds: ["sap_stake", agent_pda]
//
//  Higher stake → higher trust.  Slashable on lost disputes.
//  Unstaking has a cooldown (UNSTAKE_COOLDOWN_SLOTS ≈ 7 days).
// ─────────────────────────────────────────────────────────────────

#[derive(InitSpace)]
#[account]
pub struct AgentStake {
    pub bump: u8,
    pub agent: Pubkey,
    pub wallet: Pubkey,
    pub staked_amount: u64,
    pub slashed_amount: u64, // lifetime slashed
    pub last_stake_at: i64,
    pub unstake_requested_at: i64, // 0 = no pending unstake
    pub unstake_amount: u64,
    pub unstake_available_at: i64, // 0 = no pending unstake
    pub total_disputes_won: u32,   // disputes where agent won
    pub total_disputes_lost: u32,  // disputes where agent lost
    pub created_at: i64,
}

impl AgentStake {
    pub const MIN_STAKE: u64 = 100_000_000; // 0.1 SOL — permanent floor
    pub const UNSTAKE_COOLDOWN_SECONDS: i64 = 604_800; // 7 days (was misnamed `_SLOTS` pre-v0.11)
    /// DEPRECATED — kept for backwards-compatible IDL/clients. Use `UNSTAKE_COOLDOWN_SECONDS`.
    pub const UNSTAKE_COOLDOWN_SLOTS: u64 = 1_512_000;
    pub const SLASH_BPS: u64 = 5_000; // 50% of dispute amount
    pub const PROOF_DEADLINE_SECONDS: i64 = 604_800; // 7 days to submit receipt proof
    pub const QUALITY_DISPUTE_BOND_BPS: u64 = 1_000; // 10% bond for quality disputes
    /// v0.11 H-1: per-escrow stake-coverage ratio. Required stake at create-time
    /// is `max(MIN_STAKE, escrow_amount * STAKE_COVERAGE_BPS / 10_000)`. Set to
    /// SLASH_BPS so the slash on a lost dispute is fully collateralised by stake.
    pub const STAKE_COVERAGE_BPS: u64 = Self::SLASH_BPS;
}

// ─────────────────────────────────────────────────────────────────
//  Subscription — Recurring Payment Model
//  Seeds: ["sap_sub", agent_pda, subscriber_wallet, sub_id(u64 LE)]
//
//  Fixed-price unlimited calls for a billing interval.
//  Subscriber pre-funds; agent claims each completed interval.
//  Pro-rata refund on cancellation.
// ─────────────────────────────────────────────────────────────────

#[derive(InitSpace)]
#[account]
pub struct Subscription {
    pub bump: u8,
    pub agent: Pubkey,
    pub subscriber: Pubkey,
    pub agent_wallet: Pubkey,
    pub sub_id: u64,
    pub price_per_interval: u64,
    pub billing_interval: BillingInterval,
    pub token_mint: Option<Pubkey>,
    pub token_decimals: u8,
    pub balance: u64,
    pub total_paid: u64,
    pub intervals_paid: u32,
    pub started_at: i64,
    pub last_claimed_at: i64,
    pub cancelled_at: i64, // 0 = active
    pub next_due_at: i64,
    pub created_at: i64,
}

impl Subscription {
    pub fn interval_seconds(interval: BillingInterval) -> i64 {
        match interval {
            BillingInterval::Daily => 86_400,
            BillingInterval::Weekly => 604_800,
            BillingInterval::Monthly => 2_592_000,
        }
    }
}

// ─────────────────────────────────────────────────────────────────
//  CounterShard — Sharded Global Counters
//  Seeds: ["sap_shard", shard_index(u8)]
//
//  8 shards → 8× write throughput vs monolithic GlobalRegistry.
//  Total = sum of all shards.  SDK reads all 8 in parallel.
// ─────────────────────────────────────────────────────────────────

#[derive(InitSpace)]
#[account]
pub struct CounterShard {
    pub bump: u8,
    pub shard_index: u8,
    pub total_agents: u64,
    pub active_agents: u64,
    pub total_feedbacks: u64,
    pub total_tools: u32,
    pub total_vaults: u32,
    pub total_attestations: u32,
    pub total_settlements: u64,
    pub total_disputes: u32,
    pub total_subscriptions: u32,
    pub last_updated: i64,
}

impl CounterShard {
    pub const NUM_SHARDS: u8 = 8;
}

// ─────────────────────────────────────────────────────────────────
//  IndexPage — Overflow Pages for Discovery Indexes
//  Seeds: ["sap_idx_page", parent_index_pda, page_index(u8)]
//
//  When CapabilityIndex/ProtocolIndex/ToolCategoryIndex hits
//  MAX_AGENTS/MAX_TOOLS (100), new entries go into IndexPage PDAs.
//  Page 0 = the original index.  Pages 1..N = overflow.
// ─────────────────────────────────────────────────────────────────

#[derive(InitSpace)]
#[account]
pub struct IndexPage {
    pub bump: u8,
    pub parent_index: Pubkey,
    pub page_index: u8,
    #[max_len(100)]
    pub entries: Vec<Pubkey>,
    pub last_updated: i64,
}

impl IndexPage {
    pub const MAX_ENTRIES: usize = 100;
}

// ═══════════════════════════════════════════════════════════════════
//  v0.10 Hardening — Anti-replay receipt PDA
//
//  Created via `init` constraint inside settle_calls / settle_batch /
//  settle_calls_v2 / create_pending_settlement.  The PDA seeds
//  embed both the escrow key and the service_hash, so any attempt to
//  reuse the same `service_hash` against the same escrow fails the
//  Anchor `init` check (account already exists) — replay impossible.
//
//  Seeds:
//    settle_calls (v1):     ["sap_recv", escrow_pda, service_hash]
//    settle_calls_v2:       ["sap_recv", escrow_v2_pda, service_hash]
//    settle_batch:          ["sap_recv", escrow_pda, batch_root]
//                           (batch_root = sha256 of all service_hashes
//                            concatenated in batch order)
//
//  Cost: ~0.001 SOL per receipt.  Receipts are intentionally NOT
//  closeable to preserve the replay-protection invariant for the
//  lifetime of the escrow.  When the escrow is closed the receipts
//  become orphan PDAs whose seeds are no longer reachable through
//  any future escrow with that exact key (PDA = f(programId, escrow,
//  service_hash) — escrow PDA itself is unique per (agent, depositor)).
// ═══════════════════════════════════════════════════════════════════

#[derive(InitSpace)]
#[account]
pub struct SettlementReceipt {
    pub bump: u8,
    pub escrow: Pubkey,         // EscrowAccount or EscrowAccountV2 PDA
    pub service_hash: [u8; 32], // the (or batch_root) settled hash
    pub calls_settled: u64,     // calls settled by this receipt
    pub amount: u64,            // payout amount (escrow base unit)
    pub settled_at: i64,
}

// ═══════════════════════════════════════════════════════════════════
//  v0.10 Hardening — VaultDelegate expiry policy
// ═══════════════════════════════════════════════════════════════════

impl VaultDelegate {
    /// Maximum allowed delegate lifetime measured from now (1 year).
    /// Enforced at `add_vault_delegate` time only — pre-existing
    /// delegates with `expires_at = 0` (never) keep working.
    pub const MAX_DELEGATE_DURATION_SECS: i64 = 365 * 86_400;
}
