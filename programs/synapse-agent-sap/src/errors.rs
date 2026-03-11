use anchor_lang::prelude::*;

#[error_code]
pub enum SapError {
    // ── Agent Validation (basic) ──
    #[msg("name>64")]
    NameTooLong,

    #[msg("desc>256")]
    DescriptionTooLong,

    #[msg("uri>256")]
    UriTooLong,

    #[msg("caps>10")]
    TooManyCapabilities,

    #[msg("tiers>5")]
    TooManyPricingTiers,

    #[msg("protos>5")]
    TooManyProtocols,

    #[msg("plugins>5")]
    TooManyPlugins,

    // ── Agent State ──
    #[msg("already active")]
    AlreadyActive,

    #[msg("already inactive")]
    AlreadyInactive,

    // ── Feedback Validation ──
    #[msg("score 0-1000")]
    InvalidFeedbackScore,

    #[msg("tag>32")]
    TagTooLong,

    #[msg("self review")]
    SelfReviewNotAllowed,

    #[msg("already revoked")]
    FeedbackAlreadyRevoked,

    // ── Indexing ──
    #[msg("cap idx full")]
    CapabilityIndexFull,

    #[msg("proto idx full")]
    ProtocolIndexFull,

    #[msg("not in idx")]
    AgentNotInIndex,

    #[msg("cap hash")]
    InvalidCapabilityHash,

    #[msg("proto hash")]
    InvalidProtocolHash,

    // ── Plugin ──
    #[msg("bad plugin type")]
    InvalidPluginType,

    // ── Memory ──
    #[msg("chunk>900")]
    ChunkDataTooLarge,

    #[msg("ctype>max")]
    ContentTypeTooLong,

    #[msg("cid>max")]
    IpfsCidTooLong,

    // ── Deep Validation (Validator module) ──
    #[msg("empty name")]
    EmptyName,

    #[msg("ctrl char")]
    ControlCharInName,

    #[msg("empty desc")]
    EmptyDescription,

    #[msg("agentid>128")]
    AgentIdTooLong,

    #[msg("cap format")]
    InvalidCapabilityFormat,

    #[msg("dup cap")]
    DuplicateCapability,

    #[msg("empty tier")]
    EmptyTierId,

    #[msg("dup tier")]
    DuplicateTierId,

    #[msg("rate=0")]
    InvalidRateLimit,

    #[msg("spl needs mint")]
    SplRequiresTokenMint,

    #[msg("x402 https")]
    InvalidX402Endpoint,

    #[msg("curve order")]
    InvalidVolumeCurve,

    #[msg("curve>5")]
    TooManyVolumeCurvePoints,

    #[msg("min>max price")]
    MinPriceExceedsMax,

    #[msg("uptime 0-100")]
    InvalidUptimePercent,

    // ── Memory Vault (Encrypted Inscriptions) ──
    #[msg("session closed")]
    SessionClosed,

    #[msg("bad seq")]
    InvalidSequence,

    #[msg("frag idx")]
    InvalidFragmentIndex,

    #[msg("data>750")]
    InscriptionTooLarge,

    #[msg("empty data")]
    EmptyInscription,

    #[msg("frags<1")]
    InvalidTotalFragments,

    #[msg("epoch mismatch")]
    EpochMismatch,

    // ── Vault Lifecycle ──
    #[msg("vault open")]
    VaultNotClosed,

    #[msg("session open")]
    SessionNotClosed,

    // ── Delegation ──
    #[msg("delegate expired")]
    DelegateExpired,

    #[msg("bad delegate")]
    InvalidDelegate,

    // ── Tool Registry ──
    #[msg("tool>32")]
    ToolNameTooLong,

    #[msg("empty tool")]
    EmptyToolName,

    #[msg("tool hash")]
    InvalidToolNameHash,

    #[msg("bad method")]
    InvalidToolHttpMethod,

    #[msg("bad category")]
    InvalidToolCategory,

    #[msg("tool inactive")]
    ToolAlreadyInactive,

    #[msg("tool active")]
    ToolAlreadyActive,

    // ── Schema Inscription ──
    #[msg("schema hash")]
    InvalidSchemaHash,

    #[msg("schema type")]
    InvalidSchemaType,

    // ── Checkpoints ──
    #[msg("cp index")]
    InvalidCheckpointIndex,

    // ── Close Guards ──
    #[msg("not revoked")]
    FeedbackNotRevoked,

    #[msg("idx not empty")]
    IndexNotEmpty,

    #[msg("session open")]
    SessionStillOpen,

    // ── Update Guards ──
    #[msg("no fields")]
    NoFieldsToUpdate,

    // ── Escrow (x402 Settlement) ──
    #[msg("low balance")]
    InsufficientEscrowBalance,

    #[msg("max calls")]
    EscrowMaxCallsExceeded,

    #[msg("escrow empty")]
    EscrowEmpty,

    #[msg("escrow!=0")]
    EscrowNotEmpty,

    #[msg("calls<1")]
    InvalidSettlementCalls,

    // ── Attestation (Web of Trust) ──
    #[msg("atype>32")]
    AttestationTypeTooLong,

    #[msg("empty atype")]
    EmptyAttestationType,

    #[msg("self attest")]
    SelfAttestationNotAllowed,

    #[msg("already revoked")]
    AttestationAlreadyRevoked,

    #[msg("not revoked")]
    AttestationNotRevoked,

    // ── Tool Category Index ──
    #[msg("cat idx full")]
    ToolCategoryIndexFull,

    #[msg("not in cat")]
    ToolNotInCategoryIndex,

    #[msg("cat mismatch")]
    ToolCategoryMismatch,

    // ── Arithmetic Safety ──
    #[msg("overflow")]
    ArithmeticOverflow,

    // ── Escrow Security ──
    #[msg("escrow expired")]
    EscrowExpired,

    // ── Agent State Guard ──
    #[msg("agent inactive")]
    AgentInactive,

    // ── Attestation Security ──
    #[msg("attest expired")]
    AttestationExpired,

    // ── Memory Buffer ──
    #[msg("buf full")]
    BufferFull,

    #[msg("buf>750")]
    BufferDataTooLarge,

    #[msg("unauthorized")]
    Unauthorized,

    #[msg("bad session")]
    InvalidSession,

    // ── Memory Digest ──
    #[msg("empty hash")]
    EmptyDigestHash,

    // ── Memory Ledger ──
    #[msg("ledger>750")]
    LedgerDataTooLarge,
    #[msg("ring empty")]
    LedgerRingEmpty,

    // ── Batch Settlement ──
    #[msg("batch empty")]
    BatchEmpty,
    #[msg("batch>10")]
    BatchTooLarge,

    // ── SPL Token Escrow ──
    #[msg("spl accts")]
    SplTokenRequired,
    #[msg("bad token")]
    InvalidTokenAccount,
    #[msg("bad prog")]
    InvalidTokenProgram,
}
