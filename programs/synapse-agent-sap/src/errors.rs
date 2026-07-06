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
    #[msg("payment token not accepted (USDC only)")]
    InvalidPaymentToken,

    // ── v2.1: Escrow V2 ──
    #[msg("bad security")]
    InvalidSettlementSecurity,
    #[msg("cosigner")]
    CoSignerRequired,
    #[msg("bad cosigner")]
    InvalidCoSigner,
    #[msg("bad arbiter")]
    InvalidArbiter,
    #[msg("arbiter=0")]
    ArbiterRequired,
    #[msg("nonce reused")]
    EscrowNonceReused,

    // ── v2.1: Pending Settlement / Dispute Window ──
    #[msg("not pending")]
    SettlementNotPending,
    #[msg("already final")]
    SettlementAlreadyFinalized,
    #[msg("too early")]
    DisputeWindowNotExpired,
    #[msg("window closed")]
    DisputeWindowExpired,
    #[msg("not depositor")]
    NotDepositor,
    #[msg("dup dispute")]
    DisputeAlreadyFiled,
    #[msg("dispute open")]
    DisputeStillOpen,
    #[msg("not arbiter")]
    NotArbiter,
    #[msg("bad outcome")]
    InvalidDisputeOutcome,

    // ── v2.1: Staking ──
    #[msg("stake<min")]
    StakeBelowMinimum,
    #[msg("no stake")]
    NoStakeAccount,
    #[msg("unstake pending")]
    UnstakeAlreadyPending,
    #[msg("cooldown")]
    UnstakeCooldownNotMet,
    #[msg("no unstake")]
    NoUnstakePending,
    #[msg("slash>stake")]
    SlashExceedsStake,

    // ── v2.1: Subscription ──
    #[msg("sub active")]
    SubscriptionAlreadyActive,
    #[msg("sub cancelled")]
    SubscriptionCancelled,
    #[msg("no due")]
    NoIntervalDue,
    #[msg("sub low bal")]
    SubscriptionInsufficientBalance,
    #[msg("bad interval")]
    InvalidBillingInterval,

    // ── v2.1: Counter Shards ──
    #[msg("bad shard")]
    InvalidShardIndex,

    // ── v2.1: Index Pagination ──
    #[msg("page full")]
    IndexPageFull,
    #[msg("bad page")]
    InvalidPageIndex,
    #[msg("page≠empty")]
    IndexPageNotEmpty,

    // ── v2.1: Migration ──
    #[msg("already v2")]
    AlreadyMigrated,
    #[msg("v1 only")]
    MigrationV1Only,

    // ── v2.1: Security Fixes ──
    #[msg("disputed")]
    SettlementDisputed,
    #[msg("bad agent wallet")]
    InvalidAgentWallet,
    #[msg("stake agent mismatch")]
    StakeAgentMismatch,
    #[msg("not authority")]
    NotAuthority,
    #[msg("unstake below rent")]
    UnstakeBelowRent,
    #[msg("insufficient stake")]
    InsufficientStake,

    // ── v0.7: Receipt-Based Dispute Resolution ──
    #[msg("SelfReport deprecated")]
    SelfReportDeprecated,
    #[msg("arbiter deprecated")]
    ArbiterDeprecated,
    #[msg("bad batch idx")]
    InvalidBatchIndex,
    #[msg("bad period")]
    InvalidPeriod,
    #[msg("bad dispute type")]
    InvalidDisputeType,
    #[msg("proof expired")]
    ProofDeadlineExpired,
    #[msg("proof not expired")]
    ProofDeadlineNotExpired,
    #[msg("bad receipt proof")]
    InvalidReceiptProof,

    // ── v0.10: Hardening (audit fixes) ──
    /// service_hash already used for a previous settlement on this escrow.
    #[msg("settlement replay")]
    SettlementReplay,
    /// Token mint not allowed: only SOL (None) or USDC are accepted.
    #[msg("token not allowed")]
    PaymentTokenNotAllowed,
    /// Agent must have AgentStake PDA with at least MIN_STAKE before
    /// any new escrow can be opened against them.
    #[msg("agent stake required")]
    AgentStakeRequired,
    /// VaultDelegate.expires_at out of allowed range
    /// (must be > now and <= now + MAX_DELEGATE_DURATION).
    #[msg("delegate expiry invalid")]
    DelegateExpiryInvalid,
    /// close_agent blocked: an EscrowAccount or EscrowAccountV2(nonce=0)
    /// still exists for this (agent, wallet) pair.
    #[msg("escrow not closed")]
    EscrowNotClosed,
    /// Volume curve must be monotonically non-increasing in price
    /// (real volume discounts only — no anti-discount footgun).
    #[msg("curve not descending")]
    VolumeCurveNotDescending,
    /// Settlement batch contains a duplicated service_hash.
    #[msg("dup service hash")]
    DuplicateServiceHash,

    // ── v0.11: Staking hardening ──
    /// Stake below the per-escrow coverage requirement
    /// (stake must cover the slashable share of the new escrow).
    #[msg("stake under coverage")]
    StakeBelowCoverage,
    /// close_stake refused: stake account is not safe to close
    /// (agent still active, pending unstake, or non-floor balance).
    #[msg("stake not closable")]
    StakeNotClosable,
    /// auto_resolve_dispute requires the AgentStake account on a DepositorWins
    /// outcome to slash collateral. Caller omitted it.
    #[msg("agent stake account missing")]
    AgentStakeAccountMissing,

    // ── v0.12: Pricing menu + AgentStats hardening ──
    #[msg("requested price_per_call does not match any tier in the agent pricing menu")]
    PricingTierNotFound,
    #[msg("agent stats must be upgraded before this operation (active_escrows field missing)")]
    AgentStatsMigrationRequired,
    #[msg("pricing menu is invalid or empty (at least one tier required)")]
    InvalidPricingMenu,
    // ── v0.13 Security Hardening ──
    #[msg("calls per settlement exceeds maximum allowed (max 10000)")]
    MaxCallsPerSettlementExceeded,
    #[msg("volume curve breakpoint price must be > 0")]
    InvalidVolumeCurvePrice,
    #[msg("escrow deposit would exceed the agent's staked coverage limit")]
    EscrowCoverageExceeded,
    #[msg("co-signer cannot be the agent wallet itself")]
    CoSignerIsAgentWallet,
    #[msg("escrow has already expired")]
    EscrowAlreadyExpired,
    #[msg("escrow has an unresolved pending settlement")]
    PendingSettlementExists,
    #[msg("token account owner mismatch")]
    TokenAccountOwnerMismatch,
    #[msg("invalid protocol treasury account")]
    InvalidTreasury,
    #[msg("pending settlement PDA required")]
    PendingSettlementRequired,
    #[msg("invalid pending settlement PDA")]
    InvalidPendingSettlement,
    #[msg("create_pending_settlement is deprecated; use settle_calls_v2")]
    PendingSettlementDeprecated,
    #[msg("receipt proof exceeds maximum allowed count")]
    MaxReceiptProofExceeded,
    #[msg("merkle proof depth exceeds maximum allowed")]
    MaxMerkleDepthExceeded,
    #[msg("pending settlement amount does not match escrow pending amount")]
    PendingAmountMismatch,
    #[msg("stake slash would lock unstake request")]
    StakeSlashLocksUnstake,
    #[msg("price per call must be > 0")]
    InvalidPricePerCall,
    #[msg("subscription intervals overflow")]
    SubscriptionIntervalOverflow,
    #[msg("agent stats version mismatch — migration required")]
    AgentStatsVersionMismatch,
    #[msg("escrow version mismatch — migration required")]
    EscrowVersionMismatch,
    #[msg("receipt proof already submitted for this dispute")]
    ReceiptProofAlreadySubmitted,
    #[msg("duplicate receipt proof")]
    DuplicateReceiptProof,
    #[msg("missing verified receipt signature")]
    MissingReceiptSignature,
    #[msg("agent does not declare this capability")]
    AgentCapabilityMismatch,
    #[msg("agent does not declare this protocol")]
    AgentProtocolMismatch,
    #[msg("required params exceeds params count")]
    InvalidToolParameterCount,
    #[msg("active escrow counter underflow")]
    ActiveEscrowCounterUnderflow,
}
