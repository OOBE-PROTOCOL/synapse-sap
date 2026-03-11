use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  store_memory — Create a memory entry PDA (metadata + IPFS pointer)
//  Seeds: ["sap_memory", agent_pda, entry_hash]
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(entry_hash: [u8; 32])]
pub struct StoreMemoryAccountConstraints<'info> {
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
        space = MemoryEntry::DISCRIMINATOR.len() + MemoryEntry::INIT_SPACE,
        seeds = [b"sap_memory", agent.key().as_ref(), entry_hash.as_ref()],
        bump,
    )]
    pub memory_entry: Account<'info, MemoryEntry>,

    pub system_program: Program<'info, System>,
}

pub fn store_handler(
    ctx: Context<StoreMemoryAccountConstraints>,
    entry_hash: [u8; 32],
    content_type: String,
    ipfs_cid: Option<String>,
    total_size: u32,
) -> Result<()> {
    require!(
        content_type.len() <= MemoryEntry::MAX_CONTENT_TYPE_LEN,
        SapError::ContentTypeTooLong
    );
    if let Some(ref cid) = ipfs_cid {
        require!(
            cid.len() <= MemoryEntry::MAX_IPFS_CID_LEN,
            SapError::IpfsCidTooLong
        );
    }

    let clock = Clock::get()?;

    let entry = &mut ctx.accounts.memory_entry;
    entry.bump = ctx.bumps.memory_entry;
    entry.agent = ctx.accounts.agent.key();
    entry.entry_hash = entry_hash;
    entry.content_type = content_type.clone();
    entry.ipfs_cid = ipfs_cid;
    entry.total_chunks = 0;
    entry.total_size = total_size;
    entry.created_at = clock.unix_timestamp;
    entry.updated_at = clock.unix_timestamp;

    emit!(MemoryStoredEvent {
        agent: ctx.accounts.agent.key(),
        entry_hash,
        content_type,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  append_memory_chunk — Add a data chunk to a memory entry
//  Seeds: ["sap_mem_chunk", memory_entry_pda, chunk_index]
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(chunk_index: u8)]
pub struct AppendMemoryChunkAccountConstraints<'info> {
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
        constraint = memory_entry.agent == agent.key(),
    )]
    pub memory_entry: Account<'info, MemoryEntry>,

    #[account(
        init,
        payer = wallet,
        space = MemoryChunk::DISCRIMINATOR.len() + MemoryChunk::INIT_SPACE,
        seeds = [b"sap_mem_chunk", memory_entry.key().as_ref(), &[chunk_index]],
        bump,
    )]
    pub memory_chunk: Account<'info, MemoryChunk>,

    pub system_program: Program<'info, System>,
}

pub fn append_chunk_handler(
    ctx: Context<AppendMemoryChunkAccountConstraints>,
    chunk_index: u8,
    data: Vec<u8>,
) -> Result<()> {
    require!(
        data.len() <= MemoryChunk::MAX_CHUNK_SIZE,
        SapError::ChunkDataTooLarge
    );

    let clock = Clock::get()?;

    // ── Initialize chunk ──
    let chunk = &mut ctx.accounts.memory_chunk;
    chunk.bump = ctx.bumps.memory_chunk;
    chunk.memory_entry = ctx.accounts.memory_entry.key();
    chunk.chunk_index = chunk_index;
    chunk.data = data;

    // ── Update entry metadata ──
    let entry = &mut ctx.accounts.memory_entry;
    entry.total_chunks = entry.total_chunks
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;
    entry.updated_at = clock.unix_timestamp;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  close_memory_entry — Close a MemoryEntry PDA (rent returned)
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct CloseMemoryEntryAccountConstraints<'info> {
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
        constraint = memory_entry.agent == agent.key(),
    )]
    pub memory_entry: Account<'info, MemoryEntry>,
}

pub fn close_memory_entry_handler(_ctx: Context<CloseMemoryEntryAccountConstraints>) -> Result<()> {
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  close_memory_chunk — Close a MemoryChunk PDA (rent returned)
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct CloseMemoryChunkAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        constraint = memory_entry.agent == agent.key(),
    )]
    pub memory_entry: Account<'info, MemoryEntry>,

    #[account(
        mut,
        close = wallet,
        constraint = memory_chunk.memory_entry == memory_entry.key(),
    )]
    pub memory_chunk: Account<'info, MemoryChunk>,
}

pub fn close_memory_chunk_handler(_ctx: Context<CloseMemoryChunkAccountConstraints>) -> Result<()> {
    Ok(())
}
