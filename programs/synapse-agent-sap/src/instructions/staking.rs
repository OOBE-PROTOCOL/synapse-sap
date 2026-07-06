use crate::errors::SapError;
use crate::events::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::system_program;

// ═══════════════════════════════════════════════════════════════════
//  AGENT STAKING — Collateralized Trust Layer
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct InitStakeAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// v0.11 M-2: typed agent account — stake cannot be opened for a non-existent agent.
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init, payer = wallet,
        space = AgentStake::DISCRIMINATOR.len() + AgentStake::INIT_SPACE,
        seeds = [b"sap_stake", agent.key().as_ref()], bump,
    )]
    pub stake: Account<'info, AgentStake>,

    pub system_program: Program<'info, System>,
}

pub fn init_stake_handler(
    ctx: Context<InitStakeAccountConstraints>,
    initial_deposit: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    require!(
        initial_deposit >= AgentStake::MIN_STAKE,
        SapError::StakeBelowMinimum
    );

    // Transfer first, then mutate
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            system_program::Transfer {
                from: ctx.accounts.wallet.to_account_info(),
                to: ctx.accounts.stake.to_account_info(),
            },
        ),
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

    /// v0.11 M-2: typed agent account.
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_stake", agent.key().as_ref()], bump = stake.bump,
        constraint = stake.agent == agent.key(),
    )]
    pub stake: Account<'info, AgentStake>,

    pub system_program: Program<'info, System>,
}

pub fn deposit_stake_handler(
    ctx: Context<DepositStakeAccountConstraints>,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            system_program::Transfer {
                from: ctx.accounts.wallet.to_account_info(),
                to: ctx.accounts.stake.to_account_info(),
            },
        ),
        amount,
    )?;

    let stake = &mut ctx.accounts.stake;
    stake.staked_amount = stake
        .staked_amount
        .checked_add(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    stake.last_stake_at = clock.unix_timestamp;

    // v0.11 L-2: top-up implicitly cancels any pending unstake.
    // Emit a dedicated event so indexers can track the cancellation
    // instead of inferring it from a missing UnstakeCompletedEvent.
    let cancelled = stake.unstake_amount;
    if stake.unstake_requested_at != 0 {
        emit!(UnstakeCancelledEvent {
            agent: ctx.accounts.agent.key(),
            wallet: ctx.accounts.wallet.key(),
            cancelled_amount: cancelled,
            timestamp: clock.unix_timestamp,
        });
    }
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

    /// v0.11 M-2: typed agent account.
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_stake", agent.key().as_ref()], bump = stake.bump,
        constraint = stake.agent == agent.key(),
        constraint = stake.staked_amount > 0 @ SapError::NoStakeAccount,
        constraint = stake.unstake_requested_at == 0 @ SapError::UnstakeAlreadyPending,
    )]
    pub stake: Account<'info, AgentStake>,
}

pub fn request_unstake_handler(
    ctx: Context<RequestUnstakeAccountConstraints>,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    let stake = &mut ctx.accounts.stake;

    // L1: Support partial unstake — amount must be > 0 and <= staked
    require!(amount > 0, SapError::StakeBelowMinimum);
    require!(amount <= stake.staked_amount, SapError::InsufficientStake);

    // R1 (v0.2.0 audit pass #2): permanent collateral floor.
    // Once an agent has staked, MIN_STAKE is locked for the lifetime of the
    // AgentStake PDA — it can only be recovered by `close_stake` (v0.11 L-3).
    let remaining_after = stake
        .staked_amount
        .checked_sub(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    require!(
        remaining_after >= AgentStake::MIN_STAKE,
        SapError::StakeBelowMinimum
    );

    stake.unstake_requested_at = clock.unix_timestamp;
    stake.unstake_amount = amount;
    stake.unstake_available_at = clock
        .unix_timestamp
        .checked_add(AgentStake::UNSTAKE_COOLDOWN_SECONDS)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

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

    /// v0.11 M-2: typed agent account.
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
        has_one = wallet,
    )]
    pub agent: Account<'info, AgentAccount>,

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
    require!(
        clock.unix_timestamp >= ctx.accounts.stake.unstake_available_at,
        SapError::UnstakeCooldownNotMet
    );

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

    // v0.11 H-2: re-enforce the permanent collateral floor at completion time.
    // Between request_unstake and complete_unstake (>= 7 days) a slash may have
    // reduced staked_amount. Without this check an agent with disputes pending
    // could still drain whatever is left of the cooldown'd amount.
    let projected_stake = ctx
        .accounts
        .stake
        .staked_amount
        .saturating_sub(actual_withdraw);
    require!(
        projected_stake >= AgentStake::MIN_STAKE,
        SapError::StakeBelowMinimum
    );

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

// ══════════════════════════════════════════════════════════════════
//  close_stake — Recovery path for legacy closed agents
// ══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct CloseStakeAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA may already be closed. The PDA address is still
    /// seed-verified and must match the stake.account agent field.
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
    )]
    pub agent: UncheckedAccount<'info>,

    #[account(
        mut,
        close = wallet,
        seeds = [b"sap_stake", agent.key().as_ref()],
        bump = stake.bump,
        has_one = wallet,
        constraint = stake.agent == agent.key() @ SapError::StakeAgentMismatch,
    )]
    pub stake: Account<'info, AgentStake>,
}

pub fn close_stake_handler(ctx: Context<CloseStakeAccountConstraints>) -> Result<()> {
    require!(
        ctx.accounts.agent.data_is_empty(),
        SapError::StakeNotClosable
    );

    let ts = Clock::get()?.unix_timestamp;
    let returned_lamports = ctx.accounts.stake.to_account_info().lamports();

    emit!(StakeClosedEvent {
        agent: ctx.accounts.agent.key(),
        wallet: ctx.accounts.wallet.key(),
        returned_lamports,
        timestamp: ts,
    });

    Ok(())
}
