use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  give_feedback — Create trustless on-chain feedback
//  One feedback per (agent, reviewer) pair
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct GiveFeedbackAccountConstraints<'info> {
    #[account(mut)]
    pub reviewer: Signer<'info>,

    #[account(
        init,
        payer = reviewer,
        space = FeedbackAccount::DISCRIMINATOR.len() + FeedbackAccount::INIT_SPACE,
        seeds = [b"sap_feedback", agent.key().as_ref(), reviewer.key().as_ref()],
        bump,
    )]
    pub feedback: Account<'info, FeedbackAccount>,

    /// The agent being reviewed — must be active, reviewer must NOT be the agent owner
    #[account(
        mut,
        constraint = agent.is_active @ SapError::AgentInactive,
        constraint = reviewer.key() != agent.wallet @ SapError::SelfReviewNotAllowed,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn give_handler(
    ctx: Context<GiveFeedbackAccountConstraints>,
    score: u16,
    tag: String,
    comment_hash: Option<[u8; 32]>,
) -> Result<()> {
    require!(score <= 1000, SapError::InvalidFeedbackScore);
    require!(tag.len() <= FeedbackAccount::MAX_TAG_LEN, SapError::TagTooLong);

    let clock = Clock::get()?;

    // ── Initialize feedback PDA ──
    let feedback = &mut ctx.accounts.feedback;
    feedback.bump = ctx.bumps.feedback;
    feedback.agent = ctx.accounts.agent.key();
    feedback.reviewer = ctx.accounts.reviewer.key();
    feedback.score = score;
    feedback.tag = tag.clone();
    feedback.comment_hash = comment_hash;
    feedback.created_at = clock.unix_timestamp;
    feedback.updated_at = clock.unix_timestamp;
    feedback.is_revoked = false;

    // ── Recalculate agent reputation (incremental weighted average) ──
    let agent = &mut ctx.accounts.agent;
    agent.reputation_sum = agent.reputation_sum
        .checked_add(score as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    agent.total_feedbacks = agent.total_feedbacks
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    // reputation_score is 0-10000 (2 decimal precision)
    // score is 0-1000, so multiply by 10 for the extra decimal
    agent.reputation_score =
        ((agent.reputation_sum * 10) / agent.total_feedbacks as u64) as u32;
    agent.updated_at = clock.unix_timestamp;

    // ── Update global stats ──
    ctx.accounts.global_registry.total_feedbacks = ctx.accounts.global_registry.total_feedbacks
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    // ── Emit event ──
    emit!(FeedbackEvent {
        agent: ctx.accounts.agent.key(),
        reviewer: ctx.accounts.reviewer.key(),
        score,
        tag,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  update_feedback — Same reviewer updates their existing feedback
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct UpdateFeedbackAccountConstraints<'info> {
    pub reviewer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_feedback", agent.key().as_ref(), reviewer.key().as_ref()],
        bump = feedback.bump,
        has_one = reviewer,
        has_one = agent,
    )]
    pub feedback: Account<'info, FeedbackAccount>,

    #[account(mut)]
    pub agent: Account<'info, AgentAccount>,
}

pub fn handle_update_feedback(
    ctx: Context<UpdateFeedbackAccountConstraints>,
    new_score: u16,
    new_tag: Option<String>,
    comment_hash: Option<[u8; 32]>,
) -> Result<()> {
    require!(new_score <= 1000, SapError::InvalidFeedbackScore);
    require!(!ctx.accounts.feedback.is_revoked, SapError::FeedbackAlreadyRevoked);

    if let Some(ref t) = new_tag {
        require!(t.len() <= FeedbackAccount::MAX_TAG_LEN, SapError::TagTooLong);
    }

    let clock = Clock::get()?;
    let old_score = ctx.accounts.feedback.score;

    // ── Update feedback data ──
    let feedback = &mut ctx.accounts.feedback;
    feedback.score = new_score;
    if let Some(t) = new_tag {
        feedback.tag = t;
    }
    if comment_hash.is_some() {
        feedback.comment_hash = comment_hash;
    }
    feedback.updated_at = clock.unix_timestamp;

    // ── Recalculate agent reputation ──
    let agent = &mut ctx.accounts.agent;
    agent.reputation_sum = agent.reputation_sum
        .saturating_sub(old_score as u64)
        .checked_add(new_score as u64)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    agent.reputation_score =
        ((agent.reputation_sum * 10) / agent.total_feedbacks as u64) as u32;
    agent.updated_at = clock.unix_timestamp;

    emit!(FeedbackUpdatedEvent {
        agent: ctx.accounts.agent.key(),
        reviewer: ctx.accounts.reviewer.key(),
        old_score,
        new_score,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  revoke_feedback — Mark feedback as revoked, recalculate reputation
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct RevokeFeedbackAccountConstraints<'info> {
    pub reviewer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_feedback", agent.key().as_ref(), reviewer.key().as_ref()],
        bump = feedback.bump,
        has_one = reviewer,
        has_one = agent,
    )]
    pub feedback: Account<'info, FeedbackAccount>,

    #[account(mut)]
    pub agent: Account<'info, AgentAccount>,
}

pub fn revoke_handler(ctx: Context<RevokeFeedbackAccountConstraints>) -> Result<()> {
    require!(!ctx.accounts.feedback.is_revoked, SapError::FeedbackAlreadyRevoked);

    let clock = Clock::get()?;
    let score = ctx.accounts.feedback.score;

    // ── Mark as revoked ──
    ctx.accounts.feedback.is_revoked = true;
    ctx.accounts.feedback.updated_at = clock.unix_timestamp;

    // ── Recalculate reputation (exclude revoked feedback) ──
    let agent = &mut ctx.accounts.agent;
    agent.reputation_sum = agent.reputation_sum.saturating_sub(score as u64);
    agent.total_feedbacks = agent.total_feedbacks.saturating_sub(1);
    agent.reputation_score = if agent.total_feedbacks > 0 {
        ((agent.reputation_sum * 10) / agent.total_feedbacks as u64) as u32
    } else {
        0
    };
    agent.updated_at = clock.unix_timestamp;

    emit!(FeedbackRevokedEvent {
        agent: ctx.accounts.agent.key(),
        reviewer: ctx.accounts.reviewer.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  close_feedback — Close a revoked feedback PDA (rent returned)
//  Only revoked feedbacks can be closed.
// ═══════════════════════════════════════════════════════════════════

#[derive(Accounts)]
pub struct CloseFeedbackAccountConstraints<'info> {
    #[account(mut)]
    pub reviewer: Signer<'info>,

    #[account(
        mut,
        close = reviewer,
        seeds = [b"sap_feedback", agent.key().as_ref(), reviewer.key().as_ref()],
        bump = feedback.bump,
        has_one = reviewer,
        has_one = agent,
        constraint = feedback.is_revoked @ SapError::FeedbackNotRevoked,
    )]
    pub feedback: Account<'info, FeedbackAccount>,

    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn close_feedback_handler(ctx: Context<CloseFeedbackAccountConstraints>) -> Result<()> {
    ctx.accounts.global_registry.total_feedbacks = ctx.accounts.global_registry.total_feedbacks.saturating_sub(1);
    Ok(())
}
