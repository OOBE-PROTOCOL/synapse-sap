use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  AGENT STAKING — Collateralized Trust Layer
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct InitStakeAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified
    #[account(seeds = [b"sap_agent", wallet.key().as_ref()], bump)]
    pub agent: UncheckedAccount<'info>,

    #[account(
        init, payer = wallet,
        space = AgentStake::DISCRIMINATOR.len() + AgentStake::INIT_SPACE,
        seeds = [b"sap_stake", agent.key().as_ref()], bump,
    )]
    pub stake: Account<'info, AgentStake>,

    pub system_program: Program<'info, System>,
}

pub fn init_stake_handler(ctx: Context<InitStakeAccountConstraints>, initial_deposit: u64) -> Result<()> {
    let clock = Clock::get()?;
    require!(initial_deposit >= AgentStake::MIN_STAKE, SapError::StakeBelowMinimum);

    // Transfer first, then mutate
    system_program::transfer(
        CpiContext::new(ctx.accounts.system_program.to_account_info(), system_program::Transfer {
            from: ctx.accounts.wallet.to_account_info(),
            to: ctx.accounts.stake.to_account_info(),
        }),
        initial_deposit,
    )?;

    let stake = &mut ctx.accounts.stake;
    stake.bump = ctx.bumps.stake;
    stake.agent = ctx.accounts.agent.key();
    stake.wallet = ctx.accounts.wallet.key();
    stake.staked_amount = initial_deposit;
    stake.slashed_amount = 0;
    stake.last_stake_at = clock.unix_timestamp;
    stake.unstake_requested_at = 0;
    stake.unstake_amount = 0;
    stake.unstake_available_at = 0;
    stake.total_disputes_won = 0;
    stake.total_disputes_lost = 0;
    stake.created_at = clock.unix_timestamp;

    emit!(StakeDepositedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        amount: initial_deposit,
        total_staked: stake.staked_amount,
        timestamp: clock.unix_timestamp,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct DepositStakeAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified
    #[account(seeds = [b"sap_agent", wallet.key().as_ref()], bump)]
    pub agent: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_stake", agent.key().as_ref()], bump = stake.bump,
        constraint = stake.agent == agent.key(),
    )]
    pub stake: Account<'info, AgentStake>,

    pub system_program: Program<'info, System>,
}

pub fn deposit_stake_handler(ctx: Context<DepositStakeAccountConstraints>, amount: u64) -> Result<()> {
    let clock = Clock::get()?;
    system_program::transfer(
        CpiContext::new(ctx.accounts.system_program.to_account_info(), system_program::Transfer {
            from: ctx.accounts.wallet.to_account_info(),
            to: ctx.accounts.stake.to_account_info(),
        }),
        amount,
    )?;

    let stake = &mut ctx.accounts.stake;
    stake.staked_amount = stake.staked_amount.checked_add(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
    stake.last_stake_at = clock.unix_timestamp;
    stake.unstake_requested_at = 0;
    stake.unstake_amount = 0;
    stake.unstake_available_at = 0;

    emit!(StakeDepositedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        amount,
        total_staked: stake.staked_amount,
        timestamp: clock.unix_timestamp,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct RequestUnstakeAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified
    #[account(seeds = [b"sap_agent", wallet.key().as_ref()], bump)]
    pub agent: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_stake", agent.key().as_ref()], bump = stake.bump,
        constraint = stake.agent == agent.key(),
        constraint = stake.staked_amount > 0 @ SapError::NoStakeAccount,
        constraint = stake.unstake_requested_at == 0 @ SapError::UnstakeAlreadyPending,
    )]
    pub stake: Account<'info, AgentStake>,
}

pub fn request_unstake_handler(ctx: Context<RequestUnstakeAccountConstraints>, amount: u64) -> Result<()> {
    let clock = Clock::get()?;
    let stake = &mut ctx.accounts.stake;

    // L1: Support partial unstake — amount must be > 0 and <= staked
    require!(amount > 0, SapError::StakeBelowMinimum);
    require!(amount <= stake.staked_amount, SapError::InsufficientStake);

    stake.unstake_requested_at = clock.unix_timestamp;
    stake.unstake_amount = amount;
    stake.unstake_available_at = clock.unix_timestamp.checked_add(604_800).ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(UnstakeRequestedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: stake.wallet,
        amount,
        available_at: stake.unstake_available_at,
        timestamp: clock.unix_timestamp,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CompleteUnstakeAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified
    #[account(seeds = [b"sap_agent", wallet.key().as_ref()], bump)]
    pub agent: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_stake", agent.key().as_ref()], bump = stake.bump,
        constraint = stake.agent == agent.key(),
        constraint = stake.unstake_requested_at > 0 @ SapError::NoUnstakePending,
    )]
    pub stake: Account<'info, AgentStake>,
}

pub fn complete_unstake_handler(ctx: Context<CompleteUnstakeAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;
    require!(clock.unix_timestamp >= ctx.accounts.stake.unstake_available_at, SapError::UnstakeCooldownNotMet);

    let amount = ctx.accounts.stake.unstake_amount;
    let stake_info = ctx.accounts.stake.to_account_info();
    let wallet_info = ctx.accounts.wallet.to_account_info();

    // M4 fix: Ensure we don't withdraw below rent-exempt minimum
    let rent = Rent::get()?;
    let min_rent = rent.minimum_balance(stake_info.data_len());
    let current_lamports = stake_info.lamports();
    let max_withdraw = current_lamports.saturating_sub(min_rent);
    let actual_withdraw = amount.min(max_withdraw);
    require!(actual_withdraw > 0, SapError::UnstakeBelowRent);

    **stake_info.try_borrow_mut_lamports()? -= actual_withdraw;
    **wallet_info.try_borrow_mut_lamports()? += actual_withdraw;

    let stake = &mut ctx.accounts.stake;
    stake.staked_amount = stake.staked_amount.saturating_sub(actual_withdraw);
    stake.unstake_requested_at = 0;
    stake.unstake_amount = 0;
    stake.unstake_available_at = 0;

    emit!(UnstakeCompletedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        amount: actual_withdraw,
        remaining_staked: stake.staked_amount,
        timestamp: clock.unix_timestamp,
    });
    Ok(())
}
