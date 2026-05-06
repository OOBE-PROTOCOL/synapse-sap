use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;
use crate::validator;

// ═══════════════════════════════════════════════════════════════════
//  register_agent — Create a new agent identity PDA
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct RegisterAgentAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        init,
        payer = wallet,
        space = AgentAccount::DISCRIMINATOR.len() + AgentAccount::INIT_SPACE,
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init,
        payer = wallet,
        space = AgentStats::DISCRIMINATOR.len() + AgentStats::INIT_SPACE,
        seeds = [b"sap_stats", agent.key().as_ref()],
        bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,

    #[account(
        init,
        payer = wallet,
        space = AgentPricingMenu::DISCRIMINATOR.len() + AgentPricingMenu::INIT_SPACE,
        seeds = [b"sap_pricing", agent.key().as_ref()],
        bump,
    )]
    pub pricing_menu: Account<'info, AgentPricingMenu>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn register_handler(
    ctx: Context<RegisterAgentAccountConstraints>,
    name: String,
    description: String,
    capabilities: Vec<Capability>,
    pricing: Vec<PricingTier>,
    protocols: Vec<String>,
    agent_id: Option<String>,
    agent_uri: Option<String>,
    x402_endpoint: Option<String>,
) -> Result<()> {
    // ── Deep validation via validator module ──
    validator::validate_registration(
        &name,
        &description,
        &agent_id,
        &capabilities,
        &pricing,
        &protocols,
        &agent_uri,
        &x402_endpoint,
    )?;

    let clock = Clock::get()?;
    let cap_ids: Vec<String> = capabilities.iter().map(|c| c.id.clone()).collect();
    let agent_key = ctx.accounts.agent.key();   // cache before mutable borrow

    // ── Initialize AgentAccount PDA ──
    let agent = &mut ctx.accounts.agent;
    agent.bump = ctx.bumps.agent;
    agent.version = AgentAccount::VERSION;
    agent.wallet = ctx.accounts.wallet.key();
    agent.name = name.clone();
    agent.description = description;
    agent.agent_id = agent_id;
    agent.agent_uri = agent_uri;
    agent.x402_endpoint = x402_endpoint;
    agent.is_active = true;
    agent.created_at = clock.unix_timestamp;
    agent.updated_at = clock.unix_timestamp;
    agent.reputation_score = 0;
    agent.total_feedbacks = 0;
    agent.reputation_sum = 0;
    agent.total_calls_served = 0;
    agent.avg_latency_ms = 0;
    agent.uptime_percent = 100; // default to 100% until reported
    agent.capabilities = capabilities;
    agent.pricing = pricing;
    agent.protocols = protocols;
    agent.active_plugins = vec![];

    // ── Initialize AgentStats PDA (lightweight hot-path metrics) ──
    let stats = &mut ctx.accounts.agent_stats;
    stats.bump = ctx.bumps.agent_stats;
    stats.agent = agent_key;
    stats.wallet = ctx.accounts.wallet.key();
    stats.total_calls_served = 0;
    stats.is_active = true;
    stats.active_escrows = 0; // v0.12: counter of open escrows
    stats.updated_at = clock.unix_timestamp;

    // ── Initialize AgentPricingMenu PDA (on-chain pricing validation) ──
    let menu = &mut ctx.accounts.pricing_menu;
    menu.bump = ctx.bumps.pricing_menu;
    menu.agent = agent_key;
    menu.tiers = agent.pricing.clone();
    menu.updated_at = clock.unix_timestamp;

    // ── Update global registry ──
    let global = &mut ctx.accounts.global_registry;
    global.total_agents = global.total_agents
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    global.active_agents = global.active_agents
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    global.last_registered_at = clock.unix_timestamp;

    // ── Emit event ──
    emit!(RegisteredEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        name,
        capabilities: cap_ids,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  update_agent — Modify existing agent data (None = skip field)
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct UpdateAgentAccountConstraints<'info> {
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
        seeds = [b"sap_pricing", agent.key().as_ref()],
        bump = pricing_menu.bump,
    )]
    pub pricing_menu: Account<'info, AgentPricingMenu>,

    pub system_program: Program<'info, System>,
}

pub fn update_handler(
    ctx: Context<UpdateAgentAccountConstraints>,
    name: Option<String>,
    description: Option<String>,
    capabilities: Option<Vec<Capability>>,
    pricing: Option<Vec<PricingTier>>,
    protocols: Option<Vec<String>>,
    agent_id: Option<String>,
    agent_uri: Option<String>,
    x402_endpoint: Option<String>,
) -> Result<()> {
    // ── Deep validation via validator module ──
    validator::validate_update(
        &name,
        &description,
        &agent_id,
        &capabilities,
        &pricing,
        &protocols,
        &agent_uri,
        &x402_endpoint,
    )?;

    let agent_key = ctx.accounts.agent.key();
    let wallet_key = ctx.accounts.wallet.key();
    let agent = &mut ctx.accounts.agent;
    let mut updated_fields: Vec<String> = Vec::new();

    if let Some(n) = name {
        agent.name = n;
        updated_fields.push("name".to_string());
    }
    if let Some(d) = description {
        agent.description = d;
        updated_fields.push("description".to_string());
    }
    if let Some(id) = agent_id {
        agent.agent_id = Some(id);
        updated_fields.push("agent_id".to_string());
    }
    if let Some(caps) = capabilities {
        agent.capabilities = caps;
        updated_fields.push("capabilities".to_string());
    }
    if let Some(p) = pricing {
        agent.pricing = p.clone();
        // v0.12: sync on-chain pricing menu for escrow validation
        let menu = &mut ctx.accounts.pricing_menu;
        menu.tiers = p;
        menu.updated_at = Clock::get()?.unix_timestamp;
        updated_fields.push("pricing".to_string());
    }
    if let Some(protos) = protocols {
        agent.protocols = protos;
        updated_fields.push("protocols".to_string());
    }
    if let Some(uri) = agent_uri {
        agent.agent_uri = Some(uri);
        updated_fields.push("agent_uri".to_string());
    }
    if let Some(endpoint) = x402_endpoint {
        agent.x402_endpoint = Some(endpoint);
        updated_fields.push("x402_endpoint".to_string());
    }

    require!(!updated_fields.is_empty(), SapError::NoFieldsToUpdate);

    agent.updated_at = Clock::get()?.unix_timestamp;

    emit!(UpdatedEvent {
        agent: agent_key,
        wallet: wallet_key,
        updated_fields,
        timestamp: agent.updated_at,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  deactivate_agent — Set is_active = false
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct DeactivateAgentAccountConstraints<'info> {
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
        seeds = [b"sap_stats", agent.key().as_ref()],
        bump = agent_stats.bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn deactivate_handler(ctx: Context<DeactivateAgentAccountConstraints>) -> Result<()> {
    require!(ctx.accounts.agent.is_active, SapError::AlreadyInactive);

    let ts = Clock::get()?.unix_timestamp;
    ctx.accounts.agent.is_active = false;
    ctx.accounts.agent.updated_at = ts;
    ctx.accounts.agent_stats.is_active = false;
    ctx.accounts.agent_stats.updated_at = ts;
    ctx.accounts.global_registry.active_agents = ctx.accounts.global_registry.active_agents.saturating_sub(1);

    emit!(DeactivatedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        timestamp: ts,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  reactivate_agent — Set is_active = true
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct ReactivateAgentAccountConstraints<'info> {
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
        seeds = [b"sap_stats", agent.key().as_ref()],
        bump = agent_stats.bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn reactivate_handler(ctx: Context<ReactivateAgentAccountConstraints>) -> Result<()> {
    require!(!ctx.accounts.agent.is_active, SapError::AlreadyActive);

    let ts = Clock::get()?.unix_timestamp;
    ctx.accounts.agent.is_active = true;
    ctx.accounts.agent.updated_at = ts;
    ctx.accounts.agent_stats.is_active = true;
    ctx.accounts.agent_stats.updated_at = ts;
    ctx.accounts.global_registry.active_agents = ctx.accounts.global_registry.active_agents
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(ReactivatedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        timestamp: ts,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  close_agent — Fully close the AgentAccount PDA (rent returned)
//  Requires proof that no vault exists (or was already closed).
//  Client should remove from indexes before calling this.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct CloseAgentAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    #[account(
        mut,
        close = wallet,
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        close = wallet,
        seeds = [b"sap_stats", agent.key().as_ref()],
        bump = agent_stats.bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,

    /// CHECK: Vault PDA — must not exist. Prevents close with active vault.
    #[account(
        seeds = [b"sap_vault", agent.key().as_ref()],
        bump,
    )]
    pub vault_check: UncheckedAccount<'info>,

    #[account(
        mut,
        close = wallet,
        seeds = [b"sap_pricing", agent.key().as_ref()],
        bump,
    )]
    pub pricing_menu: Account<'info, AgentPricingMenu>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn close_handler(ctx: Context<CloseAgentAccountConstraints>) -> Result<()> {
    // Verify no active vault exists for this agent
    require!(
        ctx.accounts.vault_check.data_is_empty(),
        SapError::VaultNotClosed
    );

    // v0.12 H-1: block close if any escrow is still open for this agent.
    // An agent that closes while holding client funds is an exit-scam vector.
    require!(
        ctx.accounts.agent_stats.active_escrows == 0,
        SapError::EscrowNotClosed
    );

    let ts = Clock::get()?.unix_timestamp;
    let global = &mut ctx.accounts.global_registry;
    global.total_agents = global.total_agents.saturating_sub(1);
    if ctx.accounts.agent.is_active {
        global.active_agents = global.active_agents.saturating_sub(1);
    }

    emit!(ClosedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        timestamp: ts,
    });

    Ok(())
}
