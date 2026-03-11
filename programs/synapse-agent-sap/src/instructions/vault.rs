use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  SYNAPSE MEMORY VAULT — Encrypted Transaction Inscriptions
//
//  Data is NOT stored in PDA accounts (no rent).  Instead it's
//  emitted as an Anchor event inside the transaction log, which
//  lives on the Solana ledger permanently and is retrievable via
//  getTransaction().
//
//  Onchain we keep only:
//    • MemoryVault   (~165B)  — per-agent encryption nonce + stats
//    • SessionLedger (~150B)  — per-session sequence + epoch counter
//    • EpochPage     (~100B)  — per-epoch scan target (auto-created)
//
//  Epoch system:  Inscriptions are grouped into epochs of 1000.
//  Each epoch has its own PDA so getSignaturesForAddress() returns
//  only that epoch's TXs — enabling O(1) random access.
//
//  Cost model: ~0.004 SOL total for vault + session + epoch0 pages
//  vs. ~0.80 SOL for 100 PDA chunks.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  init_vault — Create encrypted memory vault for an agent
//  Seeds: ["sap_vault", agent_pda]
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitVaultAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init,
        payer = wallet,
        space = MemoryVault::DISCRIMINATOR.len() + MemoryVault::INIT_SPACE,
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn init_vault_handler(
    ctx: Context<InitVaultAccountConstraints>,
    vault_nonce: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;

    let vault = &mut ctx.accounts.vault;
    vault.bump = ctx.bumps.vault;
    vault.agent = ctx.accounts.agent.key();
    vault.wallet = ctx.accounts.wallet.key();
    vault.vault_nonce = vault_nonce;
    vault.total_sessions = 0;
    vault.total_inscriptions = 0;
    vault.total_bytes_inscribed = 0;
    vault.created_at = clock.unix_timestamp;
    vault.protocol_version = MemoryVault::PROTOCOL_VERSION;
    vault.nonce_version = 0;
    vault.last_nonce_rotation = 0;

    // Update global stats
    ctx.accounts.global_registry.total_vaults = ctx.accounts.global_registry.total_vaults
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(VaultInitializedEvent {
        agent: ctx.accounts.agent.key(),
        vault: vault.key(),
        wallet: ctx.accounts.wallet.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  open_session — Create a new session ledger
//  Seeds: ["sap_session", vault_pda, session_hash]
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(session_hash: [u8; 32])]
pub struct OpenSessionAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        init,
        payer = wallet,
        space = SessionLedger::DISCRIMINATOR.len() + SessionLedger::INIT_SPACE,
        seeds = [b"sap_session", vault.key().as_ref(), session_hash.as_ref()],
        bump,
    )]
    pub session: Account<'info, SessionLedger>,

    pub system_program: Program<'info, System>,
}

pub fn open_session_handler(
    ctx: Context<OpenSessionAccountConstraints>,
    session_hash: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;

    let session = &mut ctx.accounts.session;
    session.bump = ctx.bumps.session;
    session.vault = ctx.accounts.vault.key();
    session.session_hash = session_hash;
    session.sequence_counter = 0;
    session.total_bytes = 0;
    session.current_epoch = 0;
    session.total_epochs = 0;
    session.created_at = clock.unix_timestamp;
    session.last_inscribed_at = 0;
    session.is_closed = false;
    session.merkle_root = [0u8; 32];
    session.total_checkpoints = 0;
    session.tip_hash = [0u8; 32];

    let vault = &mut ctx.accounts.vault;
    vault.total_sessions = vault.total_sessions
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(SessionOpenedEvent {
        vault: vault.key(),
        session: session.key(),
        session_hash,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  inscribe_memory — Write encrypted data to TX log (ZERO RENT)
//
//  The encrypted_data travels as instruction argument → emitted
//  as MemoryInscribedEvent → stored permanently in the TX log.
//  The SessionLedger only increments a counter (no data stored).
//
//  EPOCH SYSTEM:
//  Each TX references an EpochPage PDA (auto-created via
//  init_if_needed on the first inscription of each epoch).
//  This way, getSignaturesForAddress(epochPagePDA) returns only
//  transazioni di quell'epoca — navigazione O(1).
//
//  Client calculates:  epoch_index = sequence / 1000
//  and derives the EpochPage PDA from that.
//
//  Multi-fragment support: large payloads are split client-side
//  into ≤750 byte chunks, each inscribed with incrementing sequence
//  numbers.  All fragments share the same content_hash.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(sequence: u32, encrypted_data: Vec<u8>, nonce: [u8; 12], content_hash: [u8; 32], total_fragments: u8, fragment_index: u8, compression: u8, epoch_index: u32)]
pub struct InscribeMemoryAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        mut,
        has_one = vault,
        constraint = !session.is_closed @ SapError::SessionClosed,
    )]
    pub session: Account<'info, SessionLedger>,

    /// EpochPage PDA — auto-created per epoch.
    #[account(
        init_if_needed,
        payer = wallet,
        space = EpochPage::DISCRIMINATOR.len() + EpochPage::INIT_SPACE,
        seeds = [b"sap_epoch", session.key().as_ref(), &epoch_index.to_le_bytes()],
        bump,
    )]
    pub epoch_page: Account<'info, EpochPage>,

    pub system_program: Program<'info, System>,
}

// ─────────────────────────────────────────────────────────────────
//  Shared inscription core — DRY helper for owner + delegated paths
//
//  Validates inputs, initializes epoch pages, updates merkle
//  accumulator, increments counters, and emits the event.
//  Called by both inscribe_memory_handler and
//  inscribe_memory_delegated_handler.
// ─────────────────────────────────────────────────────────────────

fn inscribe_core(
    session: &mut Account<SessionLedger>,
    vault: &mut Account<MemoryVault>,
    epoch_page: &mut Account<EpochPage>,
    epoch_page_bump: u8,
    session_key: Pubkey,
    epoch_page_key: Pubkey,
    sequence: u32,
    encrypted_data: Vec<u8>,
    nonce: [u8; 12],
    content_hash: [u8; 32],
    total_fragments: u8,
    fragment_index: u8,
    compression: u8,
    epoch_index: u32,
) -> Result<()> {
    let clock = Clock::get()?;

    // ── Validation ──
    require!(
        sequence == session.sequence_counter,
        SapError::InvalidSequence
    );
    let expected_epoch = sequence / SessionLedger::INSCRIPTIONS_PER_EPOCH;
    require!(
        epoch_index == expected_epoch,
        SapError::EpochMismatch
    );
    require!(total_fragments >= 1, SapError::InvalidTotalFragments);
    require!(
        fragment_index < total_fragments,
        SapError::InvalidFragmentIndex
    );
    require!(
        encrypted_data.len() <= SessionLedger::MAX_INSCRIPTION_SIZE,
        SapError::InscriptionTooLarge
    );
    require!(!encrypted_data.is_empty(), SapError::EmptyInscription);

    let data_len = encrypted_data.len() as u32;
    let is_new_epoch = epoch_page.session == Pubkey::default();

    // ── Initialize epoch page if new ──
    if is_new_epoch {
        epoch_page.bump = epoch_page_bump;
        epoch_page.session = session_key;
        epoch_page.epoch_index = epoch_index;
        epoch_page.start_sequence = sequence;
        epoch_page.inscription_count = 0;
        epoch_page.total_bytes = 0;
        epoch_page.first_ts = clock.unix_timestamp;
        epoch_page.last_ts = clock.unix_timestamp;

        session.current_epoch = epoch_index;
        session.total_epochs = session.total_epochs
            .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;

        emit!(EpochOpenedEvent {
            session: session_key,
            epoch_page: epoch_page_key,
            epoch_index,
            start_sequence: sequence,
            timestamp: clock.unix_timestamp,
        });
    }

    // ── Update epoch page stats ──
    epoch_page.inscription_count = epoch_page.inscription_count
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;
    epoch_page.total_bytes = epoch_page.total_bytes
        .checked_add(data_len).ok_or(error!(SapError::ArithmeticOverflow))?;
    epoch_page.last_ts = clock.unix_timestamp;

    // ── Update merkle accumulator ──
    // new_root = sha256(prev_root || content_hash)
    session.merkle_root = hashv(&[&session.merkle_root, &content_hash]).to_bytes();

    // ── Update session counter & stats (checked arithmetic) ──
    session.sequence_counter = session.sequence_counter
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    session.total_bytes = session.total_bytes
        .checked_add(data_len as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    session.last_inscribed_at = clock.unix_timestamp;
    session.tip_hash = content_hash;

    // ── Update vault aggregate stats (checked arithmetic) ──
    vault.total_inscriptions = vault.total_inscriptions
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    vault.total_bytes_inscribed = vault.total_bytes_inscribed
        .checked_add(data_len as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    // ── Emit the inscription event ──
    emit!(MemoryInscribedEvent {
        vault: vault.key(),
        session: session_key,
        sequence,
        epoch_index,
        encrypted_data,
        nonce,
        content_hash,
        total_fragments,
        fragment_index,
        compression,
        data_len,
        nonce_version: vault.nonce_version,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

pub fn inscribe_memory_handler(
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
    let session_key = ctx.accounts.session.key();
    let epoch_page_key = ctx.accounts.epoch_page.key();
    let epoch_page_bump = ctx.bumps.epoch_page;

    inscribe_core(
        &mut ctx.accounts.session,
        &mut ctx.accounts.vault,
        &mut ctx.accounts.epoch_page,
        epoch_page_bump,
        session_key,
        epoch_page_key,
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

// ─────────────────────────────────────────────────────────────────
//  close_session — Mark session as closed (no more writes)
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseSessionAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        mut,
        has_one = vault,
        constraint = !session.is_closed @ SapError::SessionClosed,
    )]
    pub session: Account<'info, SessionLedger>,
}

pub fn close_session_handler(ctx: Context<CloseSessionAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;
    let session = &mut ctx.accounts.session;

    session.is_closed = true;

    emit!(SessionClosedEvent {
        vault: ctx.accounts.vault.key(),
        session: session.key(),
        total_inscriptions: session.sequence_counter,
        total_bytes: session.total_bytes,
        total_epochs: session.total_epochs,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_vault — Close the MemoryVault PDA (rent returned)
//  Owner can reclaim rent after all usage is done.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseVaultAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        close = wallet,
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
        has_one = wallet,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn close_vault_handler(ctx: Context<CloseVaultAccountConstraints>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;

    ctx.accounts.global_registry.total_vaults = ctx.accounts.global_registry.total_vaults.saturating_sub(1);

    emit!(VaultClosedEvent {
        vault: vault.key(),
        agent: vault.agent,
        wallet: vault.wallet,
        total_sessions: vault.total_sessions,
        total_inscriptions: vault.total_inscriptions,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_session_pda — Actually close the SessionLedger PDA
//  (return rent).  Session must be marked closed first via
//  close_session.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseSessionPdaAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        mut,
        close = wallet,
        has_one = vault,
        constraint = session.is_closed @ SapError::SessionNotClosed,
    )]
    pub session: Account<'info, SessionLedger>,
}

pub fn close_session_pda_handler(ctx: Context<CloseSessionPdaAccountConstraints>) -> Result<()> {
    let session = &ctx.accounts.session;
    let clock = Clock::get()?;

    emit!(SessionPdaClosedEvent {
        vault: ctx.accounts.vault.key(),
        session: session.key(),
        total_inscriptions: session.sequence_counter,
        total_bytes: session.total_bytes,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_epoch_page — Close an EpochPage PDA (return rent)
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(epoch_index: u32)]
pub struct CloseEpochPageAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        has_one = vault,
        constraint = session.is_closed @ SapError::SessionStillOpen,
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        mut,
        close = wallet,
        seeds = [b"sap_epoch", session.key().as_ref(), &epoch_index.to_le_bytes()],
        bump = epoch_page.bump,
        has_one = session,
    )]
    pub epoch_page: Account<'info, EpochPage>,
}

pub fn close_epoch_page_handler(ctx: Context<CloseEpochPageAccountConstraints>, epoch_index: u32) -> Result<()> {
    let clock = Clock::get()?;

    emit!(EpochPageClosedEvent {
        session: ctx.accounts.session.key(),
        epoch_page: ctx.accounts.epoch_page.key(),
        epoch_index,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  rotate_vault_nonce — Update encryption nonce for future writes
//
//  Old nonce is emitted in the event so it can be recovered for
//  decrypting historical inscriptions.  nonce_version increments
//  and is included in every subsequent MemoryInscribedEvent.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct RotateVaultNonceAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
        has_one = wallet,
    )]
    pub vault: Account<'info, MemoryVault>,
}

pub fn rotate_vault_nonce_handler(
    ctx: Context<RotateVaultNonceAccountConstraints>,
    new_nonce: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;
    let vault = &mut ctx.accounts.vault;

    let old_nonce = vault.vault_nonce;
    vault.vault_nonce = new_nonce;
    vault.nonce_version = vault.nonce_version
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;
    vault.last_nonce_rotation = clock.unix_timestamp;

    emit!(VaultNonceRotatedEvent {
        vault: vault.key(),
        wallet: ctx.accounts.wallet.key(),
        old_nonce,
        new_nonce,
        nonce_version: vault.nonce_version,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  add_vault_delegate — Authorize a hot wallet for vault operations
//  Seeds: ["sap_delegate", vault_pda, delegate_pubkey]
//
//  Permissions bitmask:
//    bit 0 (1) = inscribe_memory
//    bit 1 (2) = close_session
//    bit 2 (4) = open_session
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct AddVaultDelegateAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
        has_one = wallet,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        init,
        payer = wallet,
        space = VaultDelegate::DISCRIMINATOR.len() + VaultDelegate::INIT_SPACE,
        seeds = [b"sap_delegate", vault.key().as_ref(), delegate.key().as_ref()],
        bump,
    )]
    pub vault_delegate: Account<'info, VaultDelegate>,

    /// CHECK: Delegate pubkey — no signature needed at creation.
    pub delegate: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn add_vault_delegate_handler(
    ctx: Context<AddVaultDelegateAccountConstraints>,
    permissions: u8,
    expires_at: i64,
) -> Result<()> {
    let clock = Clock::get()?;

    let del = &mut ctx.accounts.vault_delegate;
    del.bump = ctx.bumps.vault_delegate;
    del.vault = ctx.accounts.vault.key();
    del.delegate = ctx.accounts.delegate.key();
    del.permissions = permissions;
    del.expires_at = expires_at;
    del.created_at = clock.unix_timestamp;

    emit!(DelegateAddedEvent {
        vault: ctx.accounts.vault.key(),
        delegate: ctx.accounts.delegate.key(),
        permissions,
        expires_at,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  revoke_vault_delegate — Remove delegate authorization
//  PDA is closed and rent returned to the wallet owner.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct RevokeVaultDelegateAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        mut,
        close = wallet,
        seeds = [b"sap_delegate", vault.key().as_ref(), vault_delegate.delegate.as_ref()],
        bump = vault_delegate.bump,
        has_one = vault,
    )]
    pub vault_delegate: Account<'info, VaultDelegate>,
}

pub fn revoke_vault_delegate_handler(ctx: Context<RevokeVaultDelegateAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;

    emit!(DelegateRevokedEvent {
        vault: ctx.accounts.vault.key(),
        delegate: ctx.accounts.vault_delegate.delegate,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  inscribe_memory_delegated — Inscribe via authorized delegate
//
//  Same logic as inscribe_memory but the signer is a delegate
//  hot wallet instead of the vault owner.  Auth chain:
//    delegate → VaultDelegate PDA → MemoryVault → SessionLedger
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(sequence: u32, encrypted_data: Vec<u8>, nonce: [u8; 12], content_hash: [u8; 32], total_fragments: u8, fragment_index: u8, compression: u8, epoch_index: u32)]
pub struct InscribeMemoryDelegatedAccountConstraints<'info> {
    #[account(mut)]
    pub delegate_signer: Signer<'info>,

    /// Agent account — NOT derived from delegate signer.
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    /// Delegate authorization check
    #[account(
        seeds = [b"sap_delegate", vault.key().as_ref(), delegate_signer.key().as_ref()],
        bump = vault_delegate.bump,
        has_one = vault,
        constraint = vault_delegate.delegate == delegate_signer.key() @ SapError::InvalidDelegate,
    )]
    pub vault_delegate: Account<'info, VaultDelegate>,

    #[account(
        mut,
        has_one = vault,
        constraint = !session.is_closed @ SapError::SessionClosed,
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        init_if_needed,
        payer = delegate_signer,
        space = EpochPage::DISCRIMINATOR.len() + EpochPage::INIT_SPACE,
        seeds = [b"sap_epoch", session.key().as_ref(), &epoch_index.to_le_bytes()],
        bump,
    )]
    pub epoch_page: Account<'info, EpochPage>,

    pub system_program: Program<'info, System>,
}

pub fn inscribe_memory_delegated_handler(
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
    let clock = Clock::get()?;

    // ── Verify delegate permissions & expiry ──
    let del = &ctx.accounts.vault_delegate;
    require!(
        del.permissions & VaultDelegate::PERMISSION_INSCRIBE != 0,
        SapError::InvalidDelegate
    );
    if del.expires_at > 0 {
        require!(
            clock.unix_timestamp < del.expires_at,
            SapError::DelegateExpired
        );
    }

    let session_key = ctx.accounts.session.key();
    let epoch_page_key = ctx.accounts.epoch_page.key();
    let epoch_page_bump = ctx.bumps.epoch_page;

    inscribe_core(
        &mut ctx.accounts.session,
        &mut ctx.accounts.vault,
        &mut ctx.accounts.epoch_page,
        epoch_page_bump,
        session_key,
        epoch_page_key,
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

// ─────────────────────────────────────────────────────────────────
//  compact_inscribe — Simplified memory inscription (DX-first)
//
//  Designed for the common case: single-fragment, no compression.
//  Only 4 instruction args (vs 8 in inscribe_memory).
//  Does NOT create/require epoch pages — discovery via
//  getSignaturesForAddress(sessionPDA) directly.
//
//  Benefits vs inscribe_memory:
//    • 4 fewer args → smaller TX → lower priority fees
//    • No epoch_page account → saves ~0.001 SOL rent per epoch
//    • Simpler client SDK integration
//    • Same MemoryInscribedEvent format (backward compatible)
//    • Same merkle accumulator, sequence, tip_hash tracking
//
//  Tradeoff: no O(1) epoch navigation.  For sessions with
//  < 1000 inscriptions, getSignaturesForAddress(session) is fast.
//  For heavy-duty sessions (thousands of inscriptions),
//  use inscribe_memory with epoch pages instead.
//
//  SAFE to mix with inscribe_memory on the same session:
//  sequence_counter is continuous.  Compact inscriptions just
//  won't have epoch pages for their epochs.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(sequence: u32)]
pub struct CompactInscribeAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        mut,
        has_one = vault,
        constraint = !session.is_closed @ SapError::SessionClosed,
    )]
    pub session: Account<'info, SessionLedger>,
}

pub fn compact_inscribe_handler(
    ctx: Context<CompactInscribeAccountConstraints>,
    sequence: u32,
    encrypted_data: Vec<u8>,
    nonce: [u8; 12],
    content_hash: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;
    let session_key = ctx.accounts.session.key();
    let session = &mut ctx.accounts.session;
    let vault = &mut ctx.accounts.vault;

    // ── Validation ──
    require!(
        sequence == session.sequence_counter,
        SapError::InvalidSequence
    );
    require!(
        encrypted_data.len() <= SessionLedger::MAX_INSCRIPTION_SIZE,
        SapError::InscriptionTooLarge
    );
    require!(!encrypted_data.is_empty(), SapError::EmptyInscription);

    let data_len = encrypted_data.len() as u32;

    // ── Merkle accumulator ──
    session.merkle_root = hashv(&[&session.merkle_root, &content_hash]).to_bytes();

    // ── Update session (checked arithmetic) ──
    session.sequence_counter = session.sequence_counter
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    session.total_bytes = session.total_bytes
        .checked_add(data_len as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    session.last_inscribed_at = clock.unix_timestamp;
    session.tip_hash = content_hash;

    // ── Update vault (checked arithmetic) ──
    vault.total_inscriptions = vault.total_inscriptions
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    vault.total_bytes_inscribed = vault.total_bytes_inscribed
        .checked_add(data_len as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    // ── Emit inscription event (zero rent, same format) ──
    emit!(MemoryInscribedEvent {
        vault: vault.key(),
        session: session_key,
        sequence,
        epoch_index: sequence / SessionLedger::INSCRIPTIONS_PER_EPOCH,
        encrypted_data,
        nonce,
        content_hash,
        total_fragments: 1,
        fragment_index: 0,
        compression: 0,
        data_len,
        nonce_version: vault.nonce_version,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}