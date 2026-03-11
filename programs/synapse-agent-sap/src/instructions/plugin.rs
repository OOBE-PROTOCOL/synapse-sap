use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  register_plugin — Create a plugin slot PDA for an agent
//  Seeds: ["sap_plugin", agent_pda, plugin_type_u8]
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(plugin_type: u8)]
pub struct RegisterPluginAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init,
        payer = wallet,
        space = PluginSlot::DISCRIMINATOR.len() + PluginSlot::INIT_SPACE,
        seeds = [b"sap_plugin", agent.key().as_ref(), &[plugin_type]],
        bump,
    )]
    pub plugin_slot: Account<'info, PluginSlot>,

    pub system_program: Program<'info, System>,
}

pub fn handle_register_plugin(
    ctx: Context<RegisterPluginAccountConstraints>,
    plugin_type: u8,
) -> Result<()> {
    require!(
        ctx.accounts.agent.active_plugins.len() < AgentAccount::MAX_PLUGINS,
        SapError::TooManyPlugins
    );

    // Convert u8 → PluginType enum
    let pt = PluginType::from_u8(plugin_type)
        .ok_or(error!(SapError::InvalidPluginType))?;

    let clock = Clock::get()?;

    // ── Initialize plugin slot ──
    let plugin = &mut ctx.accounts.plugin_slot;
    plugin.bump = ctx.bumps.plugin_slot;
    plugin.agent = ctx.accounts.agent.key();
    plugin.plugin_type = pt;
    plugin.is_active = true;
    plugin.initialized_at = clock.unix_timestamp;
    plugin.last_updated = clock.unix_timestamp;
    plugin.data_account = None;

    // ── Add to agent's active plugins list ──
    ctx.accounts.agent.active_plugins.push(PluginRef {
        plugin_type: pt,
        pda: ctx.accounts.plugin_slot.key(),
    });
    ctx.accounts.agent.updated_at = clock.unix_timestamp;

    emit!(PluginRegisteredEvent {
        agent: ctx.accounts.agent.key(),
        plugin_type,
        plugin_pda: ctx.accounts.plugin_slot.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  close_plugin — Close a plugin slot PDA (rent returned)
//  Also removes the plugin from the agent's active_plugins list.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct ClosePluginAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        close = wallet,
        constraint = plugin_slot.agent == agent.key(),
    )]
    pub plugin_slot: Account<'info, PluginSlot>,
}

pub fn close_plugin_handler(ctx: Context<ClosePluginAccountConstraints>) -> Result<()> {
    let plugin_key = ctx.accounts.plugin_slot.key();
    let agent = &mut ctx.accounts.agent;

    // Remove from active_plugins list
    if let Some(pos) = agent.active_plugins.iter().position(|p| p.pda == plugin_key) {
        agent.active_plugins.swap_remove(pos);
    }
    agent.updated_at = Clock::get()?.unix_timestamp;

    Ok(())
}
