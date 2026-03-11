use anchor_lang::prelude::*;
use crate::state::GlobalRegistry;

/// Initialize the global SAP registry singleton.
/// Called once during program deployment.
#[derive(Accounts)]
pub struct InitializeGlobalAccountConstraints<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = GlobalRegistry::DISCRIMINATOR.len() + GlobalRegistry::INIT_SPACE,
        seeds = [b"sap_global"],
        bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn handle_initialize_global(ctx: Context<InitializeGlobalAccountConstraints>) -> Result<()> {
    let global = &mut ctx.accounts.global_registry;
    global.bump = ctx.bumps.global_registry;
    global.total_agents = 0;
    global.active_agents = 0;
    global.total_feedbacks = 0;
    global.total_capabilities = 0;
    global.total_protocols = 0;
    global.last_registered_at = 0;
    global.initialized_at = Clock::get()?.unix_timestamp;
    global.authority = ctx.accounts.authority.key();
    global.total_tools = 0;
    global.total_vaults = 0;
    global.total_escrows = 0;
    global.total_attestations = 0;

    Ok(())
}
