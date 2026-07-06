use crate::errors::SapError;
use crate::events::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::system_program;

// ═══════════════════════════════════════════════════════════════════
//  SUBSCRIPTIONS — Recurring Payment Channels
//
//  Subscriber creates a subscription to an agent with:
//    - billing_interval: Daily/Weekly/Monthly
//    - price_per_interval: fixed amount per billing cycle
//  Agent claims completed intervals.
//  Subscriber can cancel anytime (balance refunded).
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
#[instruction(sub_id: u64)]
pub struct CreateSubscriptionAccountConstraints<'info> {
    #[account(mut)]
    pub subscriber: Signer<'info>,

    #[account(constraint = agent.is_active @ SapError::AgentInactive)]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init, payer = subscriber,
        space = Subscription::DISCRIMINATOR.len() + Subscription::INIT_SPACE,
        seeds = [b"sap_sub", agent.key().as_ref(), subscriber.key().as_ref(), &sub_id.to_le_bytes()],
        bump,
    )]
    pub subscription: Account<'info, Subscription>,

    pub system_program: Program<'info, System>,
}

pub fn create_subscription_handler(
    ctx: Context<CreateSubscriptionAccountConstraints>,
    sub_id: u64,
    price_per_interval: u64,
    billing_interval: u8,
    initial_deposit: u64,
) -> Result<()> {
    let clock = Clock::get()?;

    let interval = match billing_interval {
        0 => BillingInterval::Daily,
        1 => BillingInterval::Weekly,
        2 => BillingInterval::Monthly,
        _ => return Err(error!(SapError::InvalidBillingInterval)),
    };

    let interval_secs = Subscription::interval_seconds(interval);

    if initial_deposit > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.key(),
                system_program::Transfer {
                    from: ctx.accounts.subscriber.to_account_info(),
                    to: ctx.accounts.subscription.to_account_info(),
                },
            ),
            initial_deposit,
        )?;
    }

    let sub = &mut ctx.accounts.subscription;
    sub.bump = ctx.bumps.subscription;
    sub.agent = ctx.accounts.agent.key();
    sub.subscriber = ctx.accounts.subscriber.key();
    sub.agent_wallet = ctx.accounts.agent.wallet;
    sub.sub_id = sub_id;
    sub.price_per_interval = price_per_interval;
    sub.billing_interval = interval;
    sub.token_mint = None;
    sub.token_decimals = 0;
    sub.balance = initial_deposit;
    sub.total_paid = 0;
    sub.intervals_paid = 0;
    sub.started_at = clock.unix_timestamp;
    sub.last_claimed_at = clock.unix_timestamp;
    sub.cancelled_at = 0;
    sub.next_due_at = clock
        .unix_timestamp
        .checked_add(interval_secs)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    sub.created_at = clock.unix_timestamp;

    emit!(SubscriptionCreatedEvent {
        subscription: sub.key(),
        agent: ctx.accounts.agent.key(),
        subscriber: ctx.accounts.subscriber.key(),
        sub_id,
        price_per_interval,
        billing_interval,
        initial_deposit,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct FundSubscriptionAccountConstraints<'info> {
    #[account(mut)]
    pub subscriber: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_sub", subscription.agent.as_ref(), subscriber.key().as_ref(), &subscription.sub_id.to_le_bytes()],
        bump = subscription.bump,
        constraint = subscription.subscriber == subscriber.key(),
        constraint = subscription.cancelled_at == 0 @ SapError::SubscriptionCancelled,
    )]
    pub subscription: Account<'info, Subscription>,

    pub system_program: Program<'info, System>,
}

pub fn fund_subscription_handler(
    ctx: Context<FundSubscriptionAccountConstraints>,
    amount: u64,
) -> Result<()> {
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            system_program::Transfer {
                from: ctx.accounts.subscriber.to_account_info(),
                to: ctx.accounts.subscription.to_account_info(),
            },
        ),
        amount,
    )?;

    let sub = &mut ctx.accounts.subscription;
    sub.balance = sub
        .balance
        .checked_add(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  claim_interval — Permissionless crank: claims completed intervals
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct ClaimIntervalAccountConstraints<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Agent wallet — receives payment
    #[account(
        mut,
        constraint = agent_wallet.key() == subscription.agent_wallet @ SapError::InvalidAgentWallet,
    )]
    pub agent_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_sub", subscription.agent.as_ref(), subscription.subscriber.as_ref(), &subscription.sub_id.to_le_bytes()],
        bump = subscription.bump,
        constraint = subscription.cancelled_at == 0 @ SapError::SubscriptionCancelled,
    )]
    pub subscription: Account<'info, Subscription>,
}

pub fn claim_interval_handler(ctx: Context<ClaimIntervalAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;
    let sub = &ctx.accounts.subscription;

    let interval_secs = Subscription::interval_seconds(sub.billing_interval);

    // How many intervals have elapsed since last claim?
    let elapsed = clock.unix_timestamp.saturating_sub(sub.last_claimed_at);
    let claimable = elapsed / interval_secs;
    require!(claimable > 0, SapError::NoIntervalDue);

    let total_due = (claimable as u64)
        .checked_mul(sub.price_per_interval)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    // Cap by available balance
    let actual_payment = total_due.min(sub.balance);
    // v0.13: cap claimable intervals to prevent u32 truncation in intervals_paid.
    let claimable_capped = claimable.min(u32::MAX as i64);
    let actual_intervals = if sub.price_per_interval > 0 {
        actual_payment / sub.price_per_interval
    } else {
        claimable_capped as u64
    };
    require!(
        actual_intervals > 0,
        SapError::SubscriptionInsufficientBalance
    );

    let actual_amount = actual_intervals
        .checked_mul(sub.price_per_interval)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    // Transfer: subscription PDA → agent_wallet
    let sub_info = ctx.accounts.subscription.to_account_info();
    let wallet_info = ctx.accounts.agent_wallet.to_account_info();
    **sub_info.try_borrow_mut_lamports()? -= actual_amount;
    **wallet_info.try_borrow_mut_lamports()? += actual_amount;

    let sub = &mut ctx.accounts.subscription;
    sub.balance = sub
        .balance
        .checked_sub(actual_amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    sub.total_paid = sub
        .total_paid
        .checked_add(actual_amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    sub.intervals_paid = sub
        .intervals_paid
        .checked_add(actual_intervals as u32)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    sub.last_claimed_at = sub
        .last_claimed_at
        .checked_add(actual_intervals as i64 * interval_secs)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    sub.next_due_at = sub
        .last_claimed_at
        .checked_add(interval_secs)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(SubscriptionClaimedEvent {
        subscription: sub.key(),
        agent: sub.agent,
        subscriber: sub.subscriber,
        amount: actual_amount,
        intervals_paid: sub.intervals_paid,
        remaining_balance: sub.balance,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  cancel_subscription — Subscriber cancels, refund remaining balance
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CancelSubscriptionAccountConstraints<'info> {
    #[account(mut)]
    pub subscriber: Signer<'info>,

    /// CHECK: Agent wallet — receives earned-but-unclaimed intervals
    #[account(
        mut,
        constraint = agent_wallet.key() == subscription.agent_wallet @ SapError::InvalidAgentWallet,
    )]
    pub agent_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_sub", subscription.agent.as_ref(), subscriber.key().as_ref(), &subscription.sub_id.to_le_bytes()],
        bump = subscription.bump,
        constraint = subscription.subscriber == subscriber.key(),
        constraint = subscription.cancelled_at == 0 @ SapError::SubscriptionCancelled,
    )]
    pub subscription: Account<'info, Subscription>,
}

pub fn cancel_subscription_handler(
    ctx: Context<CancelSubscriptionAccountConstraints>,
) -> Result<()> {
    let clock = Clock::get()?;

    let sub_info = ctx.accounts.subscription.to_account_info();
    let subscriber_info = ctx.accounts.subscriber.to_account_info();
    let wallet_info = ctx.accounts.agent_wallet.to_account_info();

    // M3 fix: Pay earned-but-unclaimed intervals to agent first
    let interval_secs = Subscription::interval_seconds(ctx.accounts.subscription.billing_interval);
    let elapsed = clock
        .unix_timestamp
        .saturating_sub(ctx.accounts.subscription.last_claimed_at);
    let earned_intervals = elapsed / interval_secs;
    let earned_amount = if earned_intervals > 0 && ctx.accounts.subscription.price_per_interval > 0
    {
        let due =
            (earned_intervals as u64).saturating_mul(ctx.accounts.subscription.price_per_interval);
        due.min(ctx.accounts.subscription.balance)
    } else {
        0u64
    };

    // Pay agent earned amount
    if earned_amount > 0 {
        **sub_info.try_borrow_mut_lamports()? -= earned_amount;
        **wallet_info.try_borrow_mut_lamports()? += earned_amount;
    }

    // Refund remaining to subscriber
    let refund = ctx
        .accounts
        .subscription
        .balance
        .saturating_sub(earned_amount);
    if refund > 0 {
        **sub_info.try_borrow_mut_lamports()? -= refund;
        **subscriber_info.try_borrow_mut_lamports()? += refund;
    }

    let sub = &mut ctx.accounts.subscription;
    sub.cancelled_at = clock.unix_timestamp;
    sub.total_paid = sub.total_paid.saturating_add(earned_amount);
    sub.intervals_paid = sub.intervals_paid.saturating_add(earned_intervals as u32);
    sub.balance = 0;

    emit!(SubscriptionCancelledEvent {
        subscription: sub.key(),
        agent: sub.agent,
        subscriber: ctx.accounts.subscriber.key(),
        refund_amount: refund,
        intervals_used: sub.intervals_paid,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_subscription — Reclaim rent after cancellation
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseSubscriptionAccountConstraints<'info> {
    #[account(mut)]
    pub subscriber: Signer<'info>,

    #[account(
        mut,
        close = subscriber,
        seeds = [b"sap_sub", subscription.agent.as_ref(), subscriber.key().as_ref(), &subscription.sub_id.to_le_bytes()],
        bump = subscription.bump,
        constraint = subscription.subscriber == subscriber.key(),
        constraint = subscription.cancelled_at > 0 @ SapError::SubscriptionAlreadyActive,
        constraint = subscription.balance == 0 @ SapError::SubscriptionInsufficientBalance,
    )]
    pub subscription: Account<'info, Subscription>,
}

pub fn close_subscription_handler(
    _ctx: Context<CloseSubscriptionAccountConstraints>,
) -> Result<()> {
    Ok(())
}
