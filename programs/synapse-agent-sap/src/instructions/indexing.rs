use anchor_lang::prelude::*;
use solana_sha256_hasher::hash;
use crate::state::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  init_capability_index — Create a new index PDA for a capability
//
//  ACL: requires agent ownership proof (wallet → agent PDA).
//  The first agent added to the index is the caller's own agent.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(capability_id: String, capability_hash: [u8; 32])]
pub struct InitCapabilityIndexAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// Proves caller owns this agent
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init,
        payer = wallet,
        space = CapabilityIndex::DISCRIMINATOR.len() + CapabilityIndex::INIT_SPACE,
        seeds = [b"sap_cap_idx", capability_hash.as_ref()],
        bump,
    )]
    pub capability_index: Account<'info, CapabilityIndex>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn init_capability_handler(
    ctx: Context<InitCapabilityIndexAccountConstraints>,
    capability_id: String,
    capability_hash: [u8; 32],
) -> Result<()> {
    // Verify hash matches capability_id
    let computed: [u8; 32] = hash(capability_id.as_bytes()).to_bytes();
    require!(
        computed == capability_hash,
        SapError::InvalidCapabilityHash
    );

    let agent_pda = ctx.accounts.agent.key();
    let clock = Clock::get()?;
    let index = &mut ctx.accounts.capability_index;
    index.bump = ctx.bumps.capability_index;
    index.capability_id = capability_id;
    index.capability_hash = capability_hash;
    index.agents = vec![agent_pda];
    index.total_pages = 0;
    index.last_updated = clock.unix_timestamp;

    ctx.accounts.global_registry.total_capabilities = ctx.accounts.global_registry.total_capabilities
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  add_to_capability_index — Add agent to existing index
//  ACL: only the agent owner can add their own agent.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(capability_hash: [u8; 32])]
pub struct AddToCapabilityIndexAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_cap_idx", capability_hash.as_ref()],
        bump = capability_index.bump,
    )]
    pub capability_index: Account<'info, CapabilityIndex>,
}

pub fn add_to_capability_handler(
    ctx: Context<AddToCapabilityIndexAccountConstraints>,
    _capability_hash: [u8; 32],
) -> Result<()> {
    let agent_pda = ctx.accounts.agent.key();
    let index = &mut ctx.accounts.capability_index;
    require!(
        index.agents.len() < CapabilityIndex::MAX_AGENTS,
        SapError::CapabilityIndexFull
    );

    // Prevent duplicates
    if !index.agents.contains(&agent_pda) {
        index.agents.push(agent_pda);
        index.last_updated = Clock::get()?.unix_timestamp;
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  remove_from_capability_index — Remove agent from index
//  ACL: only the agent owner can remove their own agent.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(capability_hash: [u8; 32])]
pub struct RemoveFromCapabilityIndexAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_cap_idx", capability_hash.as_ref()],
        bump = capability_index.bump,
    )]
    pub capability_index: Account<'info, CapabilityIndex>,
}

pub fn remove_from_capability_handler(
    ctx: Context<RemoveFromCapabilityIndexAccountConstraints>,
    _capability_hash: [u8; 32],
) -> Result<()> {
    let agent_pda = ctx.accounts.agent.key();
    let index = &mut ctx.accounts.capability_index;
    let pos = index
        .agents
        .iter()
        .position(|a| *a == agent_pda)
        .ok_or(SapError::AgentNotInIndex)?;

    index.agents.swap_remove(pos);
    index.last_updated = Clock::get()?.unix_timestamp;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  init_protocol_index — Create a new protocol index PDA
//  ACL: requires agent ownership proof.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(protocol_id: String, protocol_hash: [u8; 32])]
pub struct InitProtocolIndexAccountConstraints<'info> {
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
        space = ProtocolIndex::DISCRIMINATOR.len() + ProtocolIndex::INIT_SPACE,
        seeds = [b"sap_proto_idx", protocol_hash.as_ref()],
        bump,
    )]
    pub protocol_index: Account<'info, ProtocolIndex>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn init_protocol_handler(
    ctx: Context<InitProtocolIndexAccountConstraints>,
    protocol_id: String,
    protocol_hash: [u8; 32],
) -> Result<()> {
    let computed: [u8; 32] = hash(protocol_id.as_bytes()).to_bytes();
    require!(
        computed == protocol_hash,
        SapError::InvalidProtocolHash
    );

    let agent_pda = ctx.accounts.agent.key();
    let clock = Clock::get()?;
    let index = &mut ctx.accounts.protocol_index;
    index.bump = ctx.bumps.protocol_index;
    index.protocol_id = protocol_id;
    index.protocol_hash = protocol_hash;
    index.agents = vec![agent_pda];
    index.total_pages = 0;
    index.last_updated = clock.unix_timestamp;

    ctx.accounts.global_registry.total_protocols = ctx.accounts.global_registry.total_protocols
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  add_to_protocol_index — Add agent to existing protocol index
//  ACL: only the agent owner can add their own agent.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(protocol_hash: [u8; 32])]
pub struct AddToProtocolIndexAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_proto_idx", protocol_hash.as_ref()],
        bump = protocol_index.bump,
    )]
    pub protocol_index: Account<'info, ProtocolIndex>,
}

pub fn add_to_protocol_handler(
    ctx: Context<AddToProtocolIndexAccountConstraints>,
    _protocol_hash: [u8; 32],
) -> Result<()> {
    let agent_pda = ctx.accounts.agent.key();
    let index = &mut ctx.accounts.protocol_index;
    require!(
        index.agents.len() < ProtocolIndex::MAX_AGENTS,
        SapError::ProtocolIndexFull
    );

    if !index.agents.contains(&agent_pda) {
        index.agents.push(agent_pda);
        index.last_updated = Clock::get()?.unix_timestamp;
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  remove_from_protocol_index — Remove agent from protocol index
//  ACL: only the agent owner can remove their own agent.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(protocol_hash: [u8; 32])]
pub struct RemoveFromProtocolIndexAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_proto_idx", protocol_hash.as_ref()],
        bump = protocol_index.bump,
    )]
    pub protocol_index: Account<'info, ProtocolIndex>,
}

pub fn remove_from_protocol_handler(
    ctx: Context<RemoveFromProtocolIndexAccountConstraints>,
    _protocol_hash: [u8; 32],
) -> Result<()> {
    let agent_pda = ctx.accounts.agent.key();
    let index = &mut ctx.accounts.protocol_index;
    let pos = index
        .agents
        .iter()
        .position(|a| *a == agent_pda)
        .ok_or(SapError::AgentNotInIndex)?;

    index.agents.swap_remove(pos);
    index.last_updated = Clock::get()?.unix_timestamp;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  close_capability_index — Close an empty index PDA (rent returned)
//  ACL: requires agent ownership proof.
//  Guard: agents list must be empty.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(capability_hash: [u8; 32])]
pub struct CloseCapabilityIndexAccountConstraints<'info> {
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
        seeds = [b"sap_cap_idx", capability_hash.as_ref()],
        bump = capability_index.bump,
        constraint = capability_index.agents.is_empty() @ SapError::IndexNotEmpty,
    )]
    pub capability_index: Account<'info, CapabilityIndex>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn close_capability_index_handler(
    ctx: Context<CloseCapabilityIndexAccountConstraints>,
    _capability_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.global_registry.total_capabilities = ctx.accounts.global_registry.total_capabilities.saturating_sub(1);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  close_protocol_index — Close an empty protocol index PDA
//  ACL: requires agent ownership proof.
//  Guard: agents list must be empty.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(protocol_hash: [u8; 32])]
pub struct CloseProtocolIndexAccountConstraints<'info> {
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
        seeds = [b"sap_proto_idx", protocol_hash.as_ref()],
        bump = protocol_index.bump,
        constraint = protocol_index.agents.is_empty() @ SapError::IndexNotEmpty,
    )]
    pub protocol_index: Account<'info, ProtocolIndex>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn close_protocol_index_handler(
    ctx: Context<CloseProtocolIndexAccountConstraints>,
    _protocol_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.global_registry.total_protocols = ctx.accounts.global_registry.total_protocols.saturating_sub(1);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  TOOL CATEGORY INDEX — Cross-Agent Tool Discovery
//
//  Indexes ToolDescriptor PDAs by category across all agents.
//  Enables queries like "show me all Swap tools in the ecosystem"
//  or "find Data tools for price feeds".
//
//  Unlike CapabilityIndex (free-form string hash), category indices
//  have a fixed set of 10 categories (ToolCategory enum), making
//  discovery deterministic and efficient.
//
//  PDA seeds: ["sap_tool_cat", &[category_u8]]
//  → one index per category, up to 100 tools per page.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  init_tool_category_index — Create index for a category
//  Anyone with an agent can initialise it (only needed once).
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(category: u8)]
pub struct InitToolCategoryIndexAccountConstraints<'info> {
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
        space = ToolCategoryIndex::DISCRIMINATOR.len() + ToolCategoryIndex::INIT_SPACE,
        seeds = [b"sap_tool_cat".as_ref(), &[category]],
        bump,
    )]
    pub tool_category_index: Account<'info, ToolCategoryIndex>,

    pub system_program: Program<'info, System>,
}

pub fn init_tool_category_index_handler(
    ctx: Context<InitToolCategoryIndexAccountConstraints>,
    category: u8,
) -> Result<()> {
    require!(
        ToolCategory::from_u8(category).is_some(),
        SapError::InvalidToolCategory
    );

    let clock = Clock::get()?;
    let index = &mut ctx.accounts.tool_category_index;
    index.bump = ctx.bumps.tool_category_index;
    index.category = category;
    index.tools = vec![];
    index.total_pages = 0;
    index.last_updated = clock.unix_timestamp;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  add_to_tool_category — Register a tool in its category index
//  ACL: agent owner can add their own tools.
//  Guard: tool.category must match the index category.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(category: u8)]
pub struct AddToToolCategoryAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        has_one = agent,
    )]
    pub tool: Account<'info, ToolDescriptor>,

    #[account(
        mut,
        seeds = [b"sap_tool_cat".as_ref(), &[category]],
        bump = tool_category_index.bump,
    )]
    pub tool_category_index: Account<'info, ToolCategoryIndex>,
}

pub fn add_to_tool_category_handler(
    ctx: Context<AddToToolCategoryAccountConstraints>,
    category: u8,
) -> Result<()> {
    let tool = &ctx.accounts.tool;
    let index = &mut ctx.accounts.tool_category_index;

    // Verify tool category matches the index
    let expected = ToolCategory::from_u8(category)
        .ok_or(error!(SapError::InvalidToolCategory))?;
    require!(tool.category == expected, SapError::ToolCategoryMismatch);

    require!(
        index.tools.len() < ToolCategoryIndex::MAX_TOOLS,
        SapError::ToolCategoryIndexFull
    );

    let tool_key = tool.key();
    if !index.tools.contains(&tool_key) {
        index.tools.push(tool_key);
        index.last_updated = Clock::get()?.unix_timestamp;
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  remove_from_tool_category — Remove a tool from its category index
//  ACL: agent owner can remove their own tools.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(category: u8)]
pub struct RemoveFromToolCategoryAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        has_one = agent,
    )]
    pub tool: Account<'info, ToolDescriptor>,

    #[account(
        mut,
        seeds = [b"sap_tool_cat".as_ref(), &[category]],
        bump = tool_category_index.bump,
    )]
    pub tool_category_index: Account<'info, ToolCategoryIndex>,
}

pub fn remove_from_tool_category_handler(
    ctx: Context<RemoveFromToolCategoryAccountConstraints>,
    _category: u8,
) -> Result<()> {
    let tool_key = ctx.accounts.tool.key();
    let index = &mut ctx.accounts.tool_category_index;

    let pos = index
        .tools
        .iter()
        .position(|t| *t == tool_key)
        .ok_or(SapError::ToolNotInCategoryIndex)?;

    index.tools.swap_remove(pos);
    index.last_updated = Clock::get()?.unix_timestamp;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_tool_category_index — Close an empty category index PDA
//  Guard: tools list must be empty.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(category: u8)]
pub struct CloseToolCategoryIndexAccountConstraints<'info> {
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
        seeds = [b"sap_tool_cat".as_ref(), &[category]],
        bump = tool_category_index.bump,
        constraint = tool_category_index.tools.is_empty() @ SapError::IndexNotEmpty,
    )]
    pub tool_category_index: Account<'info, ToolCategoryIndex>,
}

pub fn close_tool_category_index_handler(
    _ctx: Context<CloseToolCategoryIndexAccountConstraints>,
    _category: u8,
) -> Result<()> {
    Ok(())
}
