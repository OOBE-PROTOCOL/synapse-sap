use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  SYNAPSE MEMORY DIGEST — Proof-of-Memory Protocol
//
//  The core insight: don't store DATA on-chain — store PROOF.
//  Actual data lives off-chain (IPFS, Arweave, Shadow Drive, S3).
//
//  On-chain: a FIXED-SIZE PDA (~230 bytes) that NEVER GROWS.
//  Each post_digest updates a rolling merkle root and emits an
//  event with the content_hash.  Cost = Solana TX fee only
//  (~0.000005 SOL per entry, ZERO additional rent).
//
//  Verification:
//    1. Fetch data from off-chain storage
//    2. sha256(data) → must match content_hash in TX log event
//    3. Replay merkle chain → must match on-chain merkle_root
//
//  Seeds: ["sap_digest", session_pda]
//  One digest per session.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  init_digest — Create a fixed-size proof-of-memory PDA
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitDigestAccountConstraints<'info> {
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
        constraint = !session.is_closed @ SapError::SessionClosed,
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        init,
        payer = wallet,
        space = MemoryDigest::DISCRIMINATOR.len() + MemoryDigest::INIT_SPACE,
        seeds = [b"sap_digest", session.key().as_ref()],
        bump,
    )]
    pub digest: Account<'info, MemoryDigest>,

    pub system_program: Program<'info, System>,
}

pub fn init_digest_handler(ctx: Context<InitDigestAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;

    let digest = &mut ctx.accounts.digest;
    digest.bump = ctx.bumps.digest;
    digest.session = ctx.accounts.session.key();
    digest.authority = ctx.accounts.wallet.key();
    digest.num_entries = 0;
    digest.merkle_root = [0u8; 32];
    digest.latest_hash = [0u8; 32];
    digest.total_data_size = 0;
    digest.storage_ref = [0u8; 32];
    digest.storage_type = 0;
    digest.created_at = clock.unix_timestamp;
    digest.updated_at = clock.unix_timestamp;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  post_digest — Record proof-of-memory entry
//
//  Updates rolling merkle root + counters.  ZERO additional rent
//  because the PDA never grows.  Cost = TX fee only (~0.000005 SOL).
//
//  The content_hash and data_size are emitted in DigestPostedEvent
//  for off-chain verification & indexing.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct PostDigestAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        has_one = vault,
        constraint = !session.is_closed @ SapError::SessionClosed,
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_digest", session.key().as_ref()],
        bump = digest.bump,
        constraint = digest.authority == wallet.key() @ SapError::Unauthorized,
        constraint = digest.session == session.key() @ SapError::InvalidSession,
    )]
    pub digest: Account<'info, MemoryDigest>,
}

pub fn post_digest_handler(
    ctx: Context<PostDigestAccountConstraints>,
    content_hash: [u8; 32],
    data_size: u32,
) -> Result<()> {
    let clock = Clock::get()?;

    // ── Validation ──
    require!(
        content_hash != [0u8; 32],
        SapError::EmptyDigestHash
    );

    let digest_key = ctx.accounts.digest.key();
    let digest = &mut ctx.accounts.digest;

    // ── Rolling merkle root: sha256(prev_root || content_hash) ──
    digest.merkle_root = hashv(&[&digest.merkle_root, &content_hash]).to_bytes();

    // ── Update counters (checked arithmetic) ──
    let entry_index = digest.num_entries;
    digest.num_entries = digest.num_entries
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    digest.total_data_size = digest.total_data_size
        .checked_add(data_size as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    digest.latest_hash = content_hash;
    digest.updated_at = clock.unix_timestamp;

    // ── Emit proof event (logged in TX, searchable) ──
    emit!(DigestPostedEvent {
        session: digest.session,
        digest: digest_key,
        content_hash,
        data_size,
        entry_index,
        merkle_root: digest.merkle_root,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  inscribe_to_digest — Write data ON-CHAIN + update proof
//
//  THE PRIMARY WRITE METHOD.  Combines:
//    1. Data inscription into TX log (permanent, immutable, ZERO rent)
//    2. Rolling merkle proof update in fixed PDA (ZERO growth)
//
//  Everything stays on-chain.  No off-chain dependency.
//  The digest PDA acts as BOTH proof layer AND scan index:
//    getSignaturesForAddress(digestPDA) → all write TXs
//    getTransaction(sig) → parse DigestInscribedEvent → data
//
//  Cost per write: ~0.000005 SOL (TX fee only, ZERO rent)
//  10K writes × 1KB: ~0.052 SOL total (vs ~69 SOL on-chain PDAs)
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InscribeToDigestAccountConstraints<'info> {
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
        constraint = !session.is_closed @ SapError::SessionClosed,
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        mut,
        seeds = [b"sap_digest", session.key().as_ref()],
        bump = digest.bump,
        constraint = digest.authority == wallet.key() @ SapError::Unauthorized,
        constraint = digest.session == session.key() @ SapError::InvalidSession,
    )]
    pub digest: Account<'info, MemoryDigest>,
}

pub fn inscribe_to_digest_handler(
    ctx: Context<InscribeToDigestAccountConstraints>,
    data: Vec<u8>,
    content_hash: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;

    // ── Validation ──
    require!(!data.is_empty(), SapError::EmptyInscription);
    require!(
        data.len() <= SessionLedger::MAX_INSCRIPTION_SIZE,
        SapError::InscriptionTooLarge
    );
    require!(
        content_hash != [0u8; 32],
        SapError::EmptyDigestHash
    );

    let data_len = data.len() as u32;
    let digest_key = ctx.accounts.digest.key();
    let digest = &mut ctx.accounts.digest;

    // ── Rolling merkle root: sha256(prev_root || content_hash) ──
    digest.merkle_root = hashv(&[&digest.merkle_root, &content_hash]).to_bytes();

    // ── Update counters (checked arithmetic) ──
    let entry_index = digest.num_entries;
    digest.num_entries = digest.num_entries
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    digest.total_data_size = digest.total_data_size
        .checked_add(data_len as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    digest.latest_hash = content_hash;
    digest.updated_at = clock.unix_timestamp;

    // ── Emit data into TX log (on-chain, permanent, ZERO rent) ──
    emit!(DigestInscribedEvent {
        session: digest.session,
        digest: digest_key,
        entry_index,
        data,
        content_hash,
        data_len,
        merkle_root: digest.merkle_root,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  update_digest_storage — Point to off-chain data bundle (optional)
//
//  Sets the storage_ref + storage_type so any client knows
//  WHERE to fetch the actual data.  Cost = TX fee only.
//
//  storage_type:
//    0=none, 1=IPFS, 2=Arweave, 3=ShadowDrive, 4=HTTP, 5=Filecoin
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct UpdateDigestStorageAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        has_one = vault,
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_digest", session.key().as_ref()],
        bump = digest.bump,
        constraint = digest.authority == wallet.key() @ SapError::Unauthorized,
        constraint = digest.session == session.key() @ SapError::InvalidSession,
    )]
    pub digest: Account<'info, MemoryDigest>,
}

pub fn update_digest_storage_handler(
    ctx: Context<UpdateDigestStorageAccountConstraints>,
    storage_ref: [u8; 32],
    storage_type: u8,
) -> Result<()> {
    let clock = Clock::get()?;
    let digest_key = ctx.accounts.digest.key();
    let digest = &mut ctx.accounts.digest;

    digest.storage_ref = storage_ref;
    digest.storage_type = storage_type;
    digest.updated_at = clock.unix_timestamp;

    emit!(StorageRefUpdatedEvent {
        session: digest.session,
        digest: digest_key,
        storage_ref,
        storage_type,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_digest — Close digest PDA, reclaim all rent
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseDigestAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        has_one = vault,
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, MemoryVault>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        close = wallet,
        seeds = [b"sap_digest", session.key().as_ref()],
        bump = digest.bump,
        constraint = digest.authority == wallet.key() @ SapError::Unauthorized,
        constraint = digest.session == session.key() @ SapError::InvalidSession,
    )]
    pub digest: Account<'info, MemoryDigest>,
}

pub fn close_digest_handler(_ctx: Context<CloseDigestAccountConstraints>) -> Result<()> {
    Ok(())
}
