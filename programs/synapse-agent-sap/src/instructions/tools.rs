use anchor_lang::prelude::*;
use solana_sha256_hasher::hash;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  SYNAPSE TOOL SCHEMA REGISTRY — Onchain Typed Tool Descriptors
//
//  Each tool an agent exposes gets a ToolDescriptor PDA containing
//  compact metadata + hashes of the full JSON schemas.
//
//  Full schemas are inscribed into TX logs via ToolSchemaInscribedEvent
//  (same zero-rent pattern as Memory Vault).  Any client can:
//    - Enumerate tools via getProgramAccounts(filter by agent)
//    - Verify schema integrity: sha256(schema_data) == onchain hash
//    - Walk the version chain (previous_version) for historicals
//    - Filter by category, protocol, http_method
//
//  Combined with x402 pricing on AgentAccount, this creates a fully
//  self-describing, discoverable, verifiable API surface on Solana.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  publish_tool — Register a new tool descriptor for an agent
//  Seeds: ["sap_tool", agent.key(), tool_name_hash]
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(tool_name: String, tool_name_hash: [u8; 32])]
pub struct PublishToolAccountConstraints<'info> {
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
        space = ToolDescriptor::DISCRIMINATOR.len() + ToolDescriptor::INIT_SPACE,
        seeds = [b"sap_tool", agent.key().as_ref(), tool_name_hash.as_ref()],
        bump,
    )]
    pub tool: Account<'info, ToolDescriptor>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn publish_tool_handler(
    ctx: Context<PublishToolAccountConstraints>,
    tool_name: String,
    tool_name_hash: [u8; 32],
    protocol_hash: [u8; 32],
    description_hash: [u8; 32],
    input_schema_hash: [u8; 32],
    output_schema_hash: [u8; 32],
    http_method: u8,
    category: u8,
    params_count: u8,
    required_params: u8,
    is_compound: bool,
) -> Result<()> {
    let clock = Clock::get()?;

    // ── Validation ──
    require!(!tool_name.is_empty(), SapError::EmptyToolName);
    require!(
        tool_name.len() <= ToolDescriptor::MAX_TOOL_NAME_LEN,
        SapError::ToolNameTooLong
    );

    // Verify tool_name_hash matches sha256(tool_name)
    let computed_hash = hash(tool_name.as_bytes());
    require!(
        tool_name_hash == computed_hash.to_bytes(),
        SapError::InvalidToolNameHash
    );

    // Validate enums
    require!(
        ToolHttpMethod::from_u8(http_method).is_some(),
        SapError::InvalidToolHttpMethod
    );
    require!(
        ToolCategory::from_u8(category).is_some(),
        SapError::InvalidToolCategory
    );

    let tool = &mut ctx.accounts.tool;
    tool.bump = ctx.bumps.tool;
    tool.agent = ctx.accounts.agent.key();
    tool.tool_name_hash = tool_name_hash;
    tool.tool_name = tool_name.clone();
    tool.protocol_hash = protocol_hash;
    tool.version = 1;
    tool.description_hash = description_hash;
    tool.input_schema_hash = input_schema_hash;
    tool.output_schema_hash = output_schema_hash;
    tool.http_method = ToolHttpMethod::from_u8(http_method).unwrap();
    tool.category = ToolCategory::from_u8(category).unwrap();
    tool.params_count = params_count;
    tool.required_params = required_params;
    tool.is_compound = is_compound;
    tool.is_active = true;
    tool.total_invocations = 0;
    tool.created_at = clock.unix_timestamp;
    tool.updated_at = clock.unix_timestamp;
    tool.previous_version = Pubkey::default();

    // Update global stats
    ctx.accounts.global_registry.total_tools = ctx.accounts.global_registry.total_tools
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(ToolPublishedEvent {
        agent: ctx.accounts.agent.key(),
        tool: tool.key(),
        tool_name,
        protocol_hash,
        version: 1,
        http_method,
        category,
        params_count,
        required_params,
        is_compound,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  inscribe_tool_schema — Store full JSON schema in TX logs
//
//  Same zero-rent pattern as inscribe_memory:
//  the schema_data is emitted as an event and lives in TX logs
//  permanently.  Verifiable: sha256(schema_data) must match
//  the corresponding hash in the ToolDescriptor PDA.
//
//  Call this up to 3 times per tool (input, output, description).
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InscribeToolSchemaAccountConstraints<'info> {
    #[account(mut)]
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
}

pub fn inscribe_tool_schema_handler(
    ctx: Context<InscribeToolSchemaAccountConstraints>,
    schema_type: u8,
    schema_data: Vec<u8>,
    schema_hash: [u8; 32],
    compression: u8,
) -> Result<()> {
    let clock = Clock::get()?;
    let tool = &ctx.accounts.tool;

    // schema_type: 0=input, 1=output, 2=description
    // Verify the hash matches what's on the ToolDescriptor
    // (clients re-verify: sha256(decompressed_data) == hash)
    match schema_type {
        0 => require!(schema_hash == tool.input_schema_hash, SapError::InvalidSchemaHash),
        1 => require!(schema_hash == tool.output_schema_hash, SapError::InvalidSchemaHash),
        2 => require!(schema_hash == tool.description_hash, SapError::InvalidSchemaHash),
        _ => return Err(SapError::InvalidSchemaType.into()),
    }

    emit!(ToolSchemaInscribedEvent {
        agent: ctx.accounts.agent.key(),
        tool: tool.key(),
        tool_name: tool.tool_name.clone(),
        schema_type,
        schema_data,
        schema_hash,
        compression,
        version: tool.version,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  update_tool — Bump version & update schema hashes
//
//  The previous_version field creates an immutable chain:
//  any client can walk back to find the schema for any historical
//  version of the tool.  Old schemas remain in TX logs forever.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct UpdateToolAccountConstraints<'info> {
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
        has_one = agent,
    )]
    pub tool: Account<'info, ToolDescriptor>,
}

pub fn update_tool_handler(
    ctx: Context<UpdateToolAccountConstraints>,
    description_hash: Option<[u8; 32]>,
    input_schema_hash: Option<[u8; 32]>,
    output_schema_hash: Option<[u8; 32]>,
    http_method: Option<u8>,
    category: Option<u8>,
    params_count: Option<u8>,
    required_params: Option<u8>,
) -> Result<()> {
    let clock = Clock::get()?;
    let tool = &mut ctx.accounts.tool;
    let old_version = tool.version;

    // Require at least one field to change
    require!(
        description_hash.is_some()
            || input_schema_hash.is_some()
            || output_schema_hash.is_some()
            || http_method.is_some()
            || category.is_some()
            || params_count.is_some()
            || required_params.is_some(),
        SapError::NoFieldsToUpdate
    );

    // Bump version
    tool.version = tool.version.checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;
    tool.updated_at = clock.unix_timestamp;

    // Update fields if provided
    if let Some(h) = description_hash {
        tool.description_hash = h;
    }
    if let Some(h) = input_schema_hash {
        tool.input_schema_hash = h;
    }
    if let Some(h) = output_schema_hash {
        tool.output_schema_hash = h;
    }
    if let Some(m) = http_method {
        require!(
            ToolHttpMethod::from_u8(m).is_some(),
            SapError::InvalidToolHttpMethod
        );
        tool.http_method = ToolHttpMethod::from_u8(m).unwrap();
    }
    if let Some(c) = category {
        require!(
            ToolCategory::from_u8(c).is_some(),
            SapError::InvalidToolCategory
        );
        tool.category = ToolCategory::from_u8(c).unwrap();
    }
    if let Some(p) = params_count {
        tool.params_count = p;
    }
    if let Some(r) = required_params {
        tool.required_params = r;
    }

    emit!(ToolUpdatedEvent {
        agent: ctx.accounts.agent.key(),
        tool: tool.key(),
        tool_name: tool.tool_name.clone(),
        old_version,
        new_version: tool.version,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  deactivate_tool — Mark tool as inactive (still discoverable
//  but clearly marked as not available for new calls)
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct DeactivateToolAccountConstraints<'info> {
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
        has_one = agent,
        constraint = tool.is_active @ SapError::ToolAlreadyInactive,
    )]
    pub tool: Account<'info, ToolDescriptor>,
}

pub fn deactivate_tool_handler(ctx: Context<DeactivateToolAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;
    let tool = &mut ctx.accounts.tool;

    tool.is_active = false;
    tool.updated_at = clock.unix_timestamp;

    emit!(ToolDeactivatedEvent {
        agent: ctx.accounts.agent.key(),
        tool: tool.key(),
        tool_name: tool.tool_name.clone(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  reactivate_tool — Re-enable a previously deactivated tool
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct ReactivateToolAccountConstraints<'info> {
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
        has_one = agent,
        constraint = !tool.is_active @ SapError::ToolAlreadyActive,
    )]
    pub tool: Account<'info, ToolDescriptor>,
}

pub fn reactivate_tool_handler(ctx: Context<ReactivateToolAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;
    let tool = &mut ctx.accounts.tool;

    tool.is_active = true;
    tool.updated_at = clock.unix_timestamp;

    emit!(ToolReactivatedEvent {
        agent: ctx.accounts.agent.key(),
        tool: tool.key(),
        tool_name: tool.tool_name.clone(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_tool — Close the ToolDescriptor PDA (rent returned)
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseToolAccountConstraints<'info> {
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
        has_one = agent,
    )]
    pub tool: Account<'info, ToolDescriptor>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn close_tool_handler(ctx: Context<CloseToolAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;
    let tool = &ctx.accounts.tool;

    ctx.accounts.global_registry.total_tools = ctx.accounts.global_registry.total_tools.saturating_sub(1);

    emit!(ToolClosedEvent {
        agent: ctx.accounts.agent.key(),
        tool: tool.key(),
        tool_name: tool.tool_name.clone(),
        total_invocations: tool.total_invocations,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  report_tool_invocations — Increment the invocation counter
//
//  Self-reported by the agent owner (same pattern as report_calls).
//  Useful for analytics and reputation signal.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct ReportToolInvocationsAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump = agent.bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        has_one = agent,
    )]
    pub tool: Account<'info, ToolDescriptor>,
}

pub fn report_tool_invocations_handler(
    ctx: Context<ReportToolInvocationsAccountConstraints>,
    invocations: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    let tool = &mut ctx.accounts.tool;

    tool.total_invocations = tool.total_invocations
        .checked_add(invocations)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    tool.updated_at = clock.unix_timestamp;

    emit!(ToolInvocationReportedEvent {
        agent: ctx.accounts.agent.key(),
        tool: tool.key(),
        invocations_reported: invocations,
        total_invocations: tool.total_invocations,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  create_session_checkpoint — Snapshot session state
//  Seeds: ["sap_checkpoint", session.key(), checkpoint_index(u32 LE)]
//
//  Takes a snapshot of the current merkle_root + counters.
//  Enables fast-sync: clients start from the nearest checkpoint
//  instead of replaying from genesis.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(checkpoint_index: u32)]
pub struct CreateSessionCheckpointAccountConstraints<'info> {
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
        mut,
        has_one = vault,
    )]
    pub session: Account<'info, SessionLedger>,

    #[account(
        init,
        payer = wallet,
        space = SessionCheckpoint::DISCRIMINATOR.len() + SessionCheckpoint::INIT_SPACE,
        seeds = [b"sap_checkpoint", session.key().as_ref(), &checkpoint_index.to_le_bytes()],
        bump,
    )]
    pub checkpoint: Account<'info, SessionCheckpoint>,

    pub system_program: Program<'info, System>,
}

pub fn create_session_checkpoint_handler(
    ctx: Context<CreateSessionCheckpointAccountConstraints>,
    checkpoint_index: u32,
) -> Result<()> {
    let clock = Clock::get()?;
    let session = &mut ctx.accounts.session;

    // Enforce sequential checkpoint indices
    require!(
        checkpoint_index == session.total_checkpoints,
        SapError::InvalidCheckpointIndex
    );

    let cp = &mut ctx.accounts.checkpoint;
    cp.bump = ctx.bumps.checkpoint;
    cp.session = session.key();
    cp.checkpoint_index = checkpoint_index;
    cp.merkle_root = session.merkle_root;
    cp.sequence_at = session.sequence_counter;
    cp.epoch_at = session.current_epoch;
    cp.total_bytes_at = session.total_bytes;
    cp.inscriptions_at = session.sequence_counter as u64;
    cp.created_at = clock.unix_timestamp;

    // Increment checkpoint counter
    session.total_checkpoints = session.total_checkpoints
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(CheckpointCreatedEvent {
        session: session.key(),
        checkpoint: cp.key(),
        checkpoint_index,
        merkle_root: session.merkle_root,
        sequence_at: session.sequence_counter,
        epoch_at: session.current_epoch,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_checkpoint — Close a SessionCheckpoint PDA (rent returned)
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(checkpoint_index: u32)]
pub struct CloseCheckpointAccountConstraints<'info> {
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
        seeds = [b"sap_checkpoint", session.key().as_ref(), &checkpoint_index.to_le_bytes()],
        bump = checkpoint.bump,
        has_one = session,
    )]
    pub checkpoint: Account<'info, SessionCheckpoint>,
}

pub fn close_checkpoint_handler(
    _ctx: Context<CloseCheckpointAccountConstraints>,
    _checkpoint_index: u32,
) -> Result<()> {
    Ok(())
}
