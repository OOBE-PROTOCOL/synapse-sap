use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  SYNAPSE MEMORY LEDGER — Unified On-Chain Memory
//
//  THE RECOMMENDED MEMORY SYSTEM.  Combines the best of every
//  approach into one fixed-cost PDA.
//
//  Architecture:
//    • 4KB ring buffer in the PDA — latest entries are ALWAYS
//      readable via getAccountInfo() on ANY free RPC.
//    • Every write emits a LedgerEntryEvent to the TX log —
//      data is PERMANENT, IMMUTABLE, and costs ZERO rent.
//    • Rolling merkle root proves integrity of ALL data ever written.
//
//  Two read paths for developers:
//    HOT  → getAccountInfo(ledgerPDA) → parse ring → latest msgs → FREE
//    COLD → getSignaturesForAddress(ledgerPDA) + getTransaction → full history
//
//  Cost model:
//    init_ledger    : ~0.032 SOL  (fixed, reclaimable)
//    write_ledger   : ~0.000005   (TX fee only, ZERO rent)
//    1K writes      : ~0.037 SOL  total
//    10K writes     : ~0.082 SOL  total
//    close_ledger   : reclaim ~0.032 SOL
//
//  Ring buffer entry wire format:
//    [data_len: u16 LE][data: u8 × data_len]
//
//  Seeds: ["sap_ledger", session_pda] — one ledger per session.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  init_ledger — Create a MemoryLedger with 4KB ring buffer
//
//  Fixed cost ≈0.032 SOL (reclaimable via close_ledger).
//  The PDA NEVER grows — ring buffer overwrites oldest entries.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitLedgerAccountConstraints<'info> {
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
        space = MemoryLedger::DISCRIMINATOR.len() + MemoryLedger::INIT_SPACE,
        seeds = [b"sap_ledger", session.key().as_ref()],
        bump,
    )]
    pub ledger: Account<'info, MemoryLedger>,

    pub system_program: Program<'info, System>,
}

pub fn init_ledger_handler(ctx: Context<InitLedgerAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;

    let ledger = &mut ctx.accounts.ledger;
    ledger.bump = ctx.bumps.ledger;
    ledger.session = ctx.accounts.session.key();
    ledger.authority = ctx.accounts.wallet.key();
    ledger.num_entries = 0;
    ledger.merkle_root = [0u8; 32];
    ledger.latest_hash = [0u8; 32];
    ledger.total_data_size = 0;
    ledger.created_at = clock.unix_timestamp;
    ledger.updated_at = clock.unix_timestamp;
    ledger.ring = Vec::new();
    ledger.num_pages = 0;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  write_ledger — Write data to the unified memory ledger
//
//  One instruction does THREE things simultaneously:
//    1. Emits LedgerEntryEvent to TX log (permanent, zero rent)
//    2. Updates rolling merkle root in PDA
//    3. Writes data into ring buffer (evicts oldest if full)
//
//  Cost: TX fee only (~0.000005 SOL). ZERO additional rent.
//  Max per-write data: 750 bytes (enforced on-chain).
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct WriteLedgerAccountConstraints<'info> {
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
        seeds = [b"sap_ledger", session.key().as_ref()],
        bump = ledger.bump,
        constraint = ledger.authority == wallet.key() @ SapError::Unauthorized,
        constraint = ledger.session == session.key() @ SapError::InvalidSession,
    )]
    pub ledger: Account<'info, MemoryLedger>,
}

pub fn write_ledger_handler(
    ctx: Context<WriteLedgerAccountConstraints>,
    data: Vec<u8>,
    content_hash: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;

    // ── Validation ──
    require!(!data.is_empty(), SapError::EmptyInscription);
    require!(data.len() <= 750, SapError::LedgerDataTooLarge);
    require!(content_hash != [0u8; 32], SapError::EmptyDigestHash);

    let data_len = data.len() as u32;
    let ledger_key = ctx.accounts.ledger.key();
    let ledger = &mut ctx.accounts.ledger;

    // ── Rolling merkle root: sha256(prev_root || content_hash) ──
    ledger.merkle_root = hashv(&[&ledger.merkle_root, &content_hash]).to_bytes();

    // ── Update counters (checked arithmetic) ──
    let entry_index = ledger.num_entries;
    ledger.num_entries = ledger.num_entries
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    ledger.total_data_size = ledger.total_data_size
        .checked_add(data_len as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    ledger.latest_hash = content_hash;
    ledger.updated_at = clock.unix_timestamp;

    // ── Ring buffer: sliding-window write ──
    //
    // Entry wire format: [data_len: u16 LE][data: u8 × data_len]
    //
    // When a new entry doesn't fit within RING_CAPACITY, we drain
    // the oldest entries from the front until there's room.
    // Evicted entries remain permanently in TX logs.
    let entry_size = 2 + data.len();

    while ledger.ring.len() + entry_size > MemoryLedger::RING_CAPACITY {
        if ledger.ring.len() < 2 {
            ledger.ring.clear();
            break;
        }
        let old_len = u16::from_le_bytes([ledger.ring[0], ledger.ring[1]]) as usize;
        let drain_size = 2 + old_len;
        if drain_size > ledger.ring.len() {
            // Corrupt/truncated entry — clear everything
            ledger.ring.clear();
            break;
        }
        ledger.ring.drain(..drain_size);
    }

    // Append new entry: [u16 LE length][raw data bytes]
    ledger.ring.extend_from_slice(&(data.len() as u16).to_le_bytes());
    ledger.ring.extend_from_slice(&data);

    // ── Emit to TX log — PERMANENT, IMMUTABLE, ZERO RENT ──
    emit!(LedgerEntryEvent {
        session: ledger.session,
        ledger: ledger_key,
        entry_index,
        data,
        content_hash,
        data_len,
        merkle_root: ledger.merkle_root,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_ledger — Close ledger PDA, reclaim all ~0.032 SOL rent
//
//  TX log entries remain on-chain permanently.
//  The merkle root can be reconstructed from TX log history.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseLedgerAccountConstraints<'info> {
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
        seeds = [b"sap_ledger", session.key().as_ref()],
        bump = ledger.bump,
        constraint = ledger.authority == wallet.key() @ SapError::Unauthorized,
        constraint = ledger.session == session.key() @ SapError::InvalidSession,
    )]
    pub ledger: Account<'info, MemoryLedger>,
}

pub fn close_ledger_handler(_ctx: Context<CloseLedgerAccountConstraints>) -> Result<()> {
    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  seal_ledger — Freeze ring buffer into a permanent LedgerPage
//
//  WRITE-ONCE, NEVER-DELETE.  No close instruction exists for pages.
//  The program owns the PDA and provides no way to close it.
//  Pages are PERMANENTLY and IRREVOCABLY on-chain.
//
//  This is the PROTOCOL-LEVEL GUARANTEE of immutability.
//  Even the authority cannot delete a sealed page.
//
//  Cost: ~0.031 SOL per page (the price of permanence).
//  Each page holds up to 4096 bytes of ring buffer data.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SealLedgerAccountConstraints<'info> {
    #[account(mut)]
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
        seeds = [b"sap_ledger", session.key().as_ref()],
        bump = ledger.bump,
        constraint = ledger.authority == wallet.key() @ SapError::Unauthorized,
        constraint = ledger.session == session.key() @ SapError::InvalidSession,
    )]
    pub ledger: Account<'info, MemoryLedger>,

    #[account(
        init,
        payer = wallet,
        space = LedgerPage::DISCRIMINATOR.len() + LedgerPage::INIT_SPACE,
        seeds = [b"sap_page", ledger.key().as_ref(), &ledger.num_pages.to_le_bytes()],
        bump,
    )]
    pub page: Account<'info, LedgerPage>,

    pub system_program: Program<'info, System>,
}

pub fn seal_ledger_handler(ctx: Context<SealLedgerAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;

    // ── Validate: ring must have data to seal ──
    require!(!ctx.accounts.ledger.ring.is_empty(), SapError::LedgerRingEmpty);

    // ── Snapshot ring data before mutating ──
    let ring_data = ctx.accounts.ledger.ring.clone();
    let data_size = ring_data.len() as u32;
    let merkle_snapshot = ctx.accounts.ledger.merkle_root;
    let page_index = ctx.accounts.ledger.num_pages;
    let ledger_key = ctx.accounts.ledger.key();
    let session_key = ctx.accounts.ledger.session;

    // ── Count entries in the ring (parse length-prefixed wire format) ──
    let mut entries_count: u32 = 0;
    let mut pos: usize = 0;
    while pos + 2 <= ring_data.len() {
        let len = u16::from_le_bytes([ring_data[pos], ring_data[pos + 1]]) as usize;
        if pos + 2 + len > ring_data.len() { break; }
        entries_count += 1;
        pos += 2 + len;
    }

    // ── Initialize page (WRITE-ONCE — no update/close instruction exists) ──
    let page = &mut ctx.accounts.page;
    page.bump = ctx.bumps.page;
    page.ledger = ledger_key;
    page.page_index = page_index;
    page.sealed_at = clock.unix_timestamp;
    page.entries_in_page = entries_count;
    page.data_size = data_size;
    page.merkle_root_at_seal = merkle_snapshot;
    page.data = ring_data;

    // ── Update ledger: increment page count, clear ring ──
    let ledger = &mut ctx.accounts.ledger;
    ledger.num_pages = ledger.num_pages
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    ledger.ring.clear();
    ledger.updated_at = clock.unix_timestamp;

    // ── Emit sealed event ──
    emit!(LedgerSealedEvent {
        session: session_key,
        ledger: ledger_key,
        page: ctx.accounts.page.key(),
        page_index,
        entries_in_page: entries_count,
        data_size,
        merkle_root_at_seal: merkle_snapshot,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
