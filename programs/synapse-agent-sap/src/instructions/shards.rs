use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  COUNTER SHARDS — Parallel Write Throughput
//
//  8 shards → 8× write throughput vs monolithic GlobalRegistry.
//  Seeds: ["sap_shard", &[shard_index]]
//  Total counts = Σ CounterShard[0..7] fields
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(shard_index: u8)]
pub struct InitShardAccountConstraints<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"sap_global"],
        bump = global.bump,
        constraint = global.authority == authority.key() @ SapError::NotAuthority,
    )]
    pub global: Account<'info, GlobalRegistry>,

    #[account(
        init, payer = authority,
        space = CounterShard::DISCRIMINATOR.len() + CounterShard::INIT_SPACE,
        seeds = [b"sap_shard" as &[u8], &[shard_index]],
        bump,
    )]
    pub shard: Account<'info, CounterShard>,

    pub system_program: Program<'info, System>,
}

pub fn init_shard_handler(ctx: Context<InitShardAccountConstraints>, shard_index: u8) -> Result<()> {
    require!(shard_index < CounterShard::NUM_SHARDS, SapError::InvalidShardIndex);

    let clock = Clock::get()?;
    let shard = &mut ctx.accounts.shard;
    shard.bump = ctx.bumps.shard;
    shard.shard_index = shard_index;
    shard.total_agents = 0;
    shard.active_agents = 0;
    shard.total_feedbacks = 0;
    shard.total_tools = 0;
    shard.total_vaults = 0;
    shard.total_attestations = 0;
    shard.total_settlements = 0;
    shard.total_disputes = 0;
    shard.total_subscriptions = 0;
    shard.last_updated = clock.unix_timestamp;

    emit!(ShardInitializedEvent {
        shard: shard.key(),
        shard_index,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Deterministic shard selection: first byte of key mod NUM_SHARDS
pub fn shard_index_for_key(key: &Pubkey) -> u8 {
    key.to_bytes()[0] % CounterShard::NUM_SHARDS
}
