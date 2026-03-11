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
    agent.uptime_percent = 100;   // default to 100% until reported
    agent.capabilities = capabilities;
    agent.pricing = pricing;
    agent.protocols = protocols;
    agent.active_plugins = vec![];

    // ── Initialize AgentStats PDA (lightweight hot-path metrics) ──
    let stats = &mut ctx.accounts.agent_stats;
    stats.bump = ctx.bumps.agent_stats;
    stats.agent = ctx.accounts.agent.key();
    stats.wallet = ctx.accounts.wallet.key();
    stats.total_calls_served = 0;
    stats.is_active = true;
    stats.updated_at = clock.unix_timestamp;

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
        agent.pricing = p;
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

// ═══════════════════════════════════════════════════════════════════
//  report_calls — Agent owner self-reports call metrics
//  (separate from reputation — does NOT affect reputation_score)
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct ReportCallsAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified, NOT deserialized.
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
    )]
    pub agent: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_stats", agent.key().as_ref()],
        bump = agent_stats.bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,
}

pub fn report_calls_handler(
    ctx: Context<ReportCallsAccountConstraints>,
    calls_served: u64,
) -> Result<()> {
    let stats = &mut ctx.accounts.agent_stats;
    stats.total_calls_served = stats.total_calls_served
        .checked_add(calls_served)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    stats.updated_at = Clock::get()?.unix_timestamp;

    emit!(CallsReportedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        calls_reported: calls_served,
        total_calls_served: stats.total_calls_served,
        timestamp: stats.updated_at,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  update_reputation — Agent owner self-reports latency & uptime
//  metrics. Does NOT affect feedback-based reputation_score.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct UpdateReputationAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,
}

pub fn update_reputation_handler(
    ctx: Context<UpdateReputationAccountConstraints>,
    avg_latency_ms: u32,
    uptime_percent: u8,
) -> Result<()> {
    validator::validate_uptime_percent(uptime_percent)?;

    let ts = Clock::get()?.unix_timestamp;
    let agent = &mut ctx.accounts.agent;
    agent.avg_latency_ms = avg_latency_ms;
    agent.uptime_percent = uptime_percent;
    agent.updated_at = ts;

    emit!(ReputationUpdatedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        avg_latency_ms,
        uptime_percent,
        timestamp: ts,
    });

    Ok(())
}
