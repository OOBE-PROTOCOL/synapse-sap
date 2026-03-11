use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  SYNAPSE MEMORY BUFFER — Onchain Readable Session Cache
//
//  While TX log inscriptions (inscribe_memory / compact_inscribe)
//  are permanent and rent-free, they require archival RPC access
//  (getTransaction).  MemoryBuffer provides a complementary layer:
//  data stored in PDA accounts, readable via any free RPC with a
//  simple getAccountInfo() call.
//
//  Uses Anchor's `realloc` for maximum economy:
//    • create_buffer → tiny PDA (~101 bytes, ≈0.001 SOL rent)
//    • append_buffer → grows incrementally (pay only for bytes used)
//    • close_buffer  → reclaim ALL accumulated rent
//
//  Seeds: ["sap_buffer", session_pda, page_index(u32 LE)]
//  Auth chain: wallet → agent → vault → session → buffer
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  create_buffer — Initialize a new empty buffer page
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(page_index: u32)]
pub struct CreateBufferAccountConstraints<'info> {
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
        space = MemoryBuffer::HEADER_SPACE,
        seeds = [
            b"sap_buffer",
            session.key().as_ref(),
            &page_index.to_le_bytes(),
        ],
        bump,
    )]
    pub buffer: Account<'info, MemoryBuffer>,

    pub system_program: Program<'info, System>,
}

pub fn create_buffer_handler(
    ctx: Context<CreateBufferAccountConstraints>,
    page_index: u32,
) -> Result<()> {
    let clock = Clock::get()?;
    let session_key = ctx.accounts.session.key();
    let wallet_key = ctx.accounts.wallet.key();
    let buffer_key = ctx.accounts.buffer.key();

    let buffer = &mut ctx.accounts.buffer;
    buffer.bump = ctx.bumps.buffer;
    buffer.session = session_key;
    buffer.authority = wallet_key;
    buffer.page_index = page_index;
    buffer.num_entries = 0;
    buffer.total_size = 0;
    buffer.created_at = clock.unix_timestamp;
    buffer.updated_at = clock.unix_timestamp;
    buffer.data = vec![];

    emit!(BufferCreatedEvent {
        session: session_key,
        buffer: buffer_key,
        authority: wallet_key,
        page_index,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  append_buffer — Append data to an existing buffer page
//  Uses `realloc` to dynamically grow the PDA.  Developer pays
//  additional rent proportional to data length; all reclaimable
//  via close_buffer.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(page_index: u32, data: Vec<u8>)]
pub struct AppendBufferAccountConstraints<'info> {
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
        mut,
        realloc = MemoryBuffer::HEADER_SPACE + buffer.total_size as usize + data.len(),
        realloc::payer = wallet,
        realloc::zero = false,
        seeds = [
            b"sap_buffer",
            session.key().as_ref(),
            &page_index.to_le_bytes(),
        ],
        bump = buffer.bump,
        constraint = buffer.authority == wallet.key() @ SapError::Unauthorized,
        constraint = buffer.session == session.key() @ SapError::InvalidSession,
    )]
    pub buffer: Account<'info, MemoryBuffer>,

    pub system_program: Program<'info, System>,
}

pub fn append_buffer_handler(
    ctx: Context<AppendBufferAccountConstraints>,
    _page_index: u32,
    data: Vec<u8>,
) -> Result<()> {
    let clock = Clock::get()?;

    // ── Validation ──
    require!(
        data.len() <= MemoryBuffer::MAX_WRITE_SIZE,
        SapError::BufferDataTooLarge
    );
    require!(!data.is_empty(), SapError::EmptyInscription);

    let buffer_key = ctx.accounts.buffer.key();
    let buffer = &mut ctx.accounts.buffer;

    let new_total = (buffer.total_size as usize)
        .checked_add(data.len())
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    require!(
        new_total <= MemoryBuffer::MAX_TOTAL_SIZE,
        SapError::BufferFull
    );

    // ── Append data & update metadata ──
    buffer.data.extend_from_slice(&data);
    buffer.total_size = new_total as u16;
    buffer.num_entries = buffer.num_entries
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    buffer.updated_at = clock.unix_timestamp;

    emit!(BufferAppendedEvent {
        session: buffer.session,
        buffer: buffer_key,
        page_index: buffer.page_index,
        chunk_size: data.len() as u16,
        total_size: buffer.total_size,
        num_entries: buffer.num_entries,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_buffer — Close a buffer page, reclaim ALL rent
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(page_index: u32)]
pub struct CloseBufferAccountConstraints<'info> {
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
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        mut,
        close = wallet,
        seeds = [
            b"sap_buffer",
            session.key().as_ref(),
            &page_index.to_le_bytes(),
        ],
        bump = buffer.bump,
        constraint = buffer.authority == wallet.key() @ SapError::Unauthorized,
        constraint = buffer.session == session.key() @ SapError::InvalidSession,
    )]
    pub buffer: Account<'info, MemoryBuffer>,
}

pub fn close_buffer_handler(
    _ctx: Context<CloseBufferAccountConstraints>,
    _page_index: u32,
) -> Result<()> {
    Ok(())
}
