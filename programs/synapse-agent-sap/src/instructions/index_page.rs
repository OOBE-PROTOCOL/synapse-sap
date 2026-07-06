use crate::errors::SapError;
use crate::events::*;
use crate::state::*;
use anchor_lang::prelude::*;

// ═══════════════════════════════════════════════════════════════════
//  INDEX PAGES — Overflow Discovery Indexes
//
//  Problem: Each CategoryIndex / CapabilityIndex is capped at
//  100 entries (Pubkey × 100 = 3,200 bytes).  Beyond that,
//  agents can't be discovered.
//
//  Solution: Linked overflow pages.  When page 0 fills up,
//  create page 1, etc.  Each page holds up to 100 entries.
//
//  Seeds: ["sap_idx_page", parent_index_pda, page_u8]
//
//  Pages are created on-demand by the indexing authority.
//  add/remove is called internally by register/deactivate flows.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  init_index_page — Authority creates a new overflow page
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(page_index: u8)]
pub struct InitIndexPageAccountConstraints<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"sap_global"],
        bump = global.bump,
        constraint = global.authority == authority.key() @ SapError::NotAuthority,
    )]
    pub global: Account<'info, GlobalRegistry>,

    /// The parent index account (CategoryIndex or CapabilityIndex PDA)
    /// CHECK: We use its key as seed
    pub parent_index: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        space = IndexPage::DISCRIMINATOR.len() + IndexPage::INIT_SPACE,
        seeds = [b"sap_idx_page", parent_index.key().as_ref(), &[page_index]],
        bump,
    )]
    pub index_page: Account<'info, IndexPage>,

    pub system_program: Program<'info, System>,
}

pub fn init_index_page_handler(
    ctx: Context<InitIndexPageAccountConstraints>,
    page_index: u8,
) -> Result<()> {
    let clock = Clock::get()?;
    let page = &mut ctx.accounts.index_page;

    page.bump = ctx.bumps.index_page;
    page.parent_index = ctx.accounts.parent_index.key();
    page.page_index = page_index;
    page.entries = Vec::new();
    page.last_updated = clock.unix_timestamp;

    emit!(IndexPageCreatedEvent {
        index_page: page.key(),
        parent_index: ctx.accounts.parent_index.key(),
        page_index,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  add_to_index_page — Add an agent to an overflow page
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct AddToIndexPageAccountConstraints<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"sap_global"],
        bump = global.bump,
        constraint = global.authority == authority.key() @ SapError::NotAuthority,
    )]
    pub global: Account<'info, GlobalRegistry>,

    #[account(
        mut,
        seeds = [b"sap_idx_page", index_page.parent_index.as_ref(), &[index_page.page_index]],
        bump = index_page.bump,
        constraint = index_page.entries.len() < IndexPage::MAX_ENTRIES @ SapError::IndexPageFull,
    )]
    pub index_page: Account<'info, IndexPage>,
}

pub fn add_to_index_page_handler(
    ctx: Context<AddToIndexPageAccountConstraints>,
    agent_pda: Pubkey,
) -> Result<()> {
    let page = &mut ctx.accounts.index_page;

    // Prevent duplicates
    if !page.entries.contains(&agent_pda) {
        page.entries.push(agent_pda);
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  remove_from_index_page — Remove an agent from overflow page
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct RemoveFromIndexPageAccountConstraints<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"sap_global"],
        bump = global.bump,
        constraint = global.authority == authority.key() @ SapError::NotAuthority,
    )]
    pub global: Account<'info, GlobalRegistry>,

    #[account(
        mut,
        seeds = [b"sap_idx_page", index_page.parent_index.as_ref(), &[index_page.page_index]],
        bump = index_page.bump,
    )]
    pub index_page: Account<'info, IndexPage>,
}

pub fn remove_from_index_page_handler(
    ctx: Context<RemoveFromIndexPageAccountConstraints>,
    agent_pda: Pubkey,
) -> Result<()> {
    let page = &mut ctx.accounts.index_page;

    if let Some(pos) = page.entries.iter().position(|k| *k == agent_pda) {
        page.entries.swap_remove(pos);
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_index_page — Reclaim rent from empty page
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseIndexPageAccountConstraints<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"sap_global"],
        bump = global.bump,
        constraint = global.authority == authority.key() @ SapError::NotAuthority,
    )]
    pub global: Account<'info, GlobalRegistry>,

    #[account(
        mut,
        close = authority,
        seeds = [b"sap_idx_page", index_page.parent_index.as_ref(), &[index_page.page_index]],
        bump = index_page.bump,
        constraint = index_page.entries.is_empty() @ SapError::IndexPageNotEmpty,
    )]
    pub index_page: Account<'info, IndexPage>,
}

pub fn close_index_page_handler(_ctx: Context<CloseIndexPageAccountConstraints>) -> Result<()> {
    Ok(())
}
