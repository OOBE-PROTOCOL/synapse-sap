use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  DISPUTE RESOLUTION — On-Chain Arbiter-Mediated Disputes
//
//  Flow:
//    1. Agent calls settle_calls_v2 (DisputeWindow mode)
//    2. Depositor reviews pending settlement
//    3. If contested → file_dispute() within dispute window
//    4. Arbiter reviews evidence → resolve_dispute()
//    5. Outcome: DepositorWins (refund) or AgentWins (release)
//
//  Slash mechanics:
//    - If DepositorWins AND agent has stake → slash 50% of dispute amount
//    - Slashed amount transferred to depositor as compensation
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  file_dispute — Depositor contests a pending settlement
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct FileDisputeAccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        seeds = [b"sap_escrow_v2", escrow.agent.as_ref(), depositor.key().as_ref(), &escrow.escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
        has_one = depositor,
        constraint = escrow.settlement_security == SettlementSecurity::DisputeWindow @ SapError::InvalidSettlementSecurity,
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    #[account(
        mut,
        seeds = [b"sap_pending", escrow.key().as_ref(), &pending_settlement.settlement_index.to_le_bytes()],
        bump = pending_settlement.bump,
        constraint = pending_settlement.escrow == escrow.key(),
        constraint = !pending_settlement.is_finalized @ SapError::SettlementAlreadyFinalized,
        constraint = pending_settlement.outcome == DisputeOutcome::Pending @ SapError::DisputeAlreadyFiled,
    )]
    pub pending_settlement: Account<'info, PendingSettlement>,

    #[account(
        init,
        payer = depositor,
        space = DisputeRecord::DISCRIMINATOR.len() + DisputeRecord::INIT_SPACE,
        seeds = [b"sap_dispute", pending_settlement.key().as_ref()],
        bump,
    )]
    pub dispute: Account<'info, DisputeRecord>,

    pub system_program: Program<'info, System>,
}

pub fn file_dispute_handler(
    ctx: Context<FileDisputeAccountConstraints>,
    evidence_hash: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Must be within dispute window
    require!(
        current_slot < ctx.accounts.pending_settlement.release_slot,
        SapError::DisputeWindowExpired
    );

    let arbiter = ctx.accounts.escrow.arbiter.ok_or(error!(SapError::ArbiterRequired))?;

    let dispute = &mut ctx.accounts.dispute;
    dispute.bump = ctx.bumps.dispute;
    dispute.pending_settlement = ctx.accounts.pending_settlement.key();
    dispute.escrow = ctx.accounts.escrow.key();
    dispute.depositor = ctx.accounts.depositor.key();
    dispute.agent = ctx.accounts.escrow.agent;
    dispute.arbiter = arbiter;
    dispute.evidence_hash = evidence_hash;
    dispute.agent_evidence_hash = [0u8; 32];
    dispute.outcome = DisputeOutcome::Pending;
    dispute.resolution_hash = [0u8; 32];
    dispute.resolved_at = 0;
    dispute.created_at = clock.unix_timestamp;
    dispute.slash_amount = 0;

    // Mark PendingSettlement as disputed so finalize_settlement is blocked
    ctx.accounts.pending_settlement.is_disputed = true;

    emit!(DisputeFiledEvent {
        dispute: dispute.key(),
        pending_settlement: ctx.accounts.pending_settlement.key(),
        escrow: ctx.accounts.escrow.key(),
        depositor: ctx.accounts.depositor.key(),
        agent: ctx.accounts.escrow.agent,
        evidence_hash,
        arbiter,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  submit_agent_evidence — Agent submits counter-evidence
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SubmitAgentEvidenceAccountConstraints<'info> {
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified.
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
    )]
    pub agent: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_dispute", dispute.pending_settlement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.agent == agent.key(),
        constraint = dispute.outcome == DisputeOutcome::Pending @ SapError::SettlementAlreadyFinalized,
    )]
    pub dispute: Account<'info, DisputeRecord>,
}

pub fn submit_agent_evidence_handler(
    ctx: Context<SubmitAgentEvidenceAccountConstraints>,
    evidence_hash: [u8; 32],
) -> Result<()> {
    // Agent counter-evidence stored separately — depositor evidence preserved
    ctx.accounts.dispute.agent_evidence_hash = evidence_hash;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  resolve_dispute — Arbiter resolves the dispute
//
//  outcome:
//    2 = DepositorWins → refund pending amount to depositor
//    3 = AgentWins    → release pending amount to agent
//
//  If DepositorWins and AgentStake exists, slash up to 50%
//  of the dispute amount from the agent's stake.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct ResolveDisputeAccountConstraints<'info> {
    #[account(mut)]
    pub arbiter: Signer<'info>,

    /// CHECK: Depositor receives refund if they win
    #[account(mut)]
    pub depositor: UncheckedAccount<'info>,

    /// CHECK: Agent wallet receives funds if they win
    #[account(mut)]
    pub agent_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_escrow_v2", escrow.agent.as_ref(), escrow.depositor.as_ref(), &escrow.escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    #[account(
        mut,
        seeds = [b"sap_pending", escrow.key().as_ref(), &pending_settlement.settlement_index.to_le_bytes()],
        bump = pending_settlement.bump,
        constraint = pending_settlement.escrow == escrow.key(),
        constraint = !pending_settlement.is_finalized @ SapError::SettlementAlreadyFinalized,
    )]
    pub pending_settlement: Account<'info, PendingSettlement>,

    #[account(
        mut,
        seeds = [b"sap_dispute", pending_settlement.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.arbiter == arbiter.key() @ SapError::NotArbiter,
        constraint = dispute.outcome == DisputeOutcome::Pending @ SapError::SettlementAlreadyFinalized,
    )]
    pub dispute: Account<'info, DisputeRecord>,

    #[account(
        mut,
        seeds = [b"sap_stats", escrow.agent.as_ref()],
        bump = agent_stats.bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,
}

pub fn resolve_dispute_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ResolveDisputeAccountConstraints<'info>>,
    outcome: u8, // 2=DepositorWins, 3=AgentWins
) -> Result<()> {
    let clock = Clock::get()?;

    let dispute_outcome = match outcome {
        2 => DisputeOutcome::DepositorWins,
        3 => DisputeOutcome::AgentWins,
        _ => return Err(error!(SapError::InvalidDisputeOutcome)),
    };

    // Verify depositor / agent_wallet
    require!(
        ctx.accounts.depositor.key() == ctx.accounts.escrow.depositor,
        SapError::NotDepositor
    );
    require!(
        ctx.accounts.agent_wallet.key() == ctx.accounts.escrow.agent_wallet,
        SapError::InvalidAgentWallet
    );

    let amount = ctx.accounts.pending_settlement.amount;
    let calls = ctx.accounts.pending_settlement.calls_to_settle;

    let escrow_info = ctx.accounts.escrow.to_account_info();
    let depositor_info = ctx.accounts.depositor.to_account_info();
    let wallet_info = ctx.accounts.agent_wallet.to_account_info();

    match dispute_outcome {
        DisputeOutcome::DepositorWins => {
            // Refund to depositor (move from locked → depositor)
            if ctx.accounts.escrow.token_mint.is_some() {
                let remaining = ctx.remaining_accounts;
                // M5 fix: Verify dest token account owner is depositor
                require!(remaining.len() >= 3, SapError::SplTokenRequired);
                let dest_data = remaining[1].try_borrow_data()?;
                require!(dest_data.len() >= 64, SapError::InvalidTokenAccount);
                let dest_owner = Pubkey::try_from(&dest_data[32..64])
                    .map_err(|_| error!(SapError::InvalidTokenAccount))?;
                require!(dest_owner == ctx.accounts.escrow.depositor, SapError::InvalidTokenAccount);
                drop(dest_data);

                super::escrow_v2::spl_transfer_from_escrow_v2(
                    &escrow_info, remaining,
                    &ctx.accounts.escrow.agent, &ctx.accounts.escrow.depositor,
                    ctx.accounts.escrow.escrow_nonce, ctx.accounts.escrow.bump,
                    amount, ctx.accounts.escrow.token_mint,
                )?;
            } else {
                **escrow_info.try_borrow_mut_lamports()? -= amount;
                **depositor_info.try_borrow_mut_lamports()? += amount;
            }

            let escrow = &mut ctx.accounts.escrow;
            escrow.balance = escrow.balance.checked_sub(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.pending_amount = escrow.pending_amount.checked_sub(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.pending_calls = escrow.pending_calls.checked_sub(calls).ok_or(error!(SapError::ArithmeticOverflow))?;

            // Try to slash agent stake if it exists in remaining accounts
            let slash_amount = try_slash_from_remaining(
                ctx.remaining_accounts,
                amount,
                &depositor_info,
                &ctx.accounts.escrow.agent,
                &ctx.accounts.dispute.key(),
                &clock,
            )?;

            ctx.accounts.dispute.slash_amount = slash_amount;
        }
        DisputeOutcome::AgentWins => {
            // Release to agent
            if ctx.accounts.escrow.token_mint.is_some() {
                let remaining = ctx.remaining_accounts;
                // M5 fix: Verify dest token account owner is agent_wallet
                require!(remaining.len() >= 3, SapError::SplTokenRequired);
                let dest_data = remaining[1].try_borrow_data()?;
                require!(dest_data.len() >= 64, SapError::InvalidTokenAccount);
                let dest_owner = Pubkey::try_from(&dest_data[32..64])
                    .map_err(|_| error!(SapError::InvalidTokenAccount))?;
                require!(dest_owner == ctx.accounts.escrow.agent_wallet, SapError::InvalidTokenAccount);
                drop(dest_data);

                super::escrow_v2::spl_transfer_from_escrow_v2(
                    &escrow_info, remaining,
                    &ctx.accounts.escrow.agent, &ctx.accounts.escrow.depositor,
                    ctx.accounts.escrow.escrow_nonce, ctx.accounts.escrow.bump,
                    amount, ctx.accounts.escrow.token_mint,
                )?;
            } else {
                **escrow_info.try_borrow_mut_lamports()? -= amount;
                **wallet_info.try_borrow_mut_lamports()? += amount;
            }

            let escrow = &mut ctx.accounts.escrow;
            escrow.balance = escrow.balance.checked_sub(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.total_settled = escrow.total_settled.checked_add(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.total_calls_settled = escrow.total_calls_settled.checked_add(calls).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.pending_amount = escrow.pending_amount.checked_sub(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.pending_calls = escrow.pending_calls.checked_sub(calls).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.last_settled_at = clock.unix_timestamp;

            let stats = &mut ctx.accounts.agent_stats;
            stats.total_calls_served = stats.total_calls_served.checked_add(calls).ok_or(error!(SapError::ArithmeticOverflow))?;
            stats.updated_at = clock.unix_timestamp;
        }
        _ => return Err(error!(SapError::InvalidDisputeOutcome)),
    }

    // Finalize
    let ps = &mut ctx.accounts.pending_settlement;
    ps.is_finalized = true;
    ps.outcome = dispute_outcome;

    let dispute = &mut ctx.accounts.dispute;
    dispute.outcome = dispute_outcome;
    dispute.resolved_at = clock.unix_timestamp;

    emit!(DisputeResolvedEvent {
        dispute: dispute.key(),
        pending_settlement: ps.key(),
        escrow: ctx.accounts.escrow.key(),
        outcome,
        slash_amount: dispute.slash_amount,
        resolution_hash: dispute.resolution_hash,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_dispute — Reclaim rent from finalized dispute
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseDisputeAccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        close = depositor,
        seeds = [b"sap_dispute", dispute.pending_settlement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.depositor == depositor.key() @ SapError::NotDepositor,
        constraint = dispute.outcome != DisputeOutcome::Pending @ SapError::DisputeStillOpen,
    )]
    pub dispute: Account<'info, DisputeRecord>,
}

pub fn close_dispute_handler(_ctx: Context<CloseDisputeAccountConstraints>) -> Result<()> {
    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_pending_settlement — Reclaim rent from finalized pending
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct ClosePendingSettlementAccountConstraints<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        close = payer,
        seeds = [b"sap_pending", pending_settlement.escrow.as_ref(), &pending_settlement.settlement_index.to_le_bytes()],
        bump = pending_settlement.bump,
        constraint = pending_settlement.is_finalized @ SapError::SettlementNotPending,
    )]
    pub pending_settlement: Account<'info, PendingSettlement>,
}

pub fn close_pending_settlement_handler(_ctx: Context<ClosePendingSettlementAccountConstraints>) -> Result<()> {
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Internal: Slash from AgentStake typed account in remaining
//
//  Fixed C1: Uses proper Anchor deserialization, verifies stake.agent
//  matches escrow.agent, updates all relevant fields, emits event.
// ═══════════════════════════════════════════════════════════════════

fn try_slash_from_remaining<'info>(
    remaining: &'info [AccountInfo<'info>],
    dispute_amount: u64,
    depositor: &AccountInfo<'info>,
    expected_agent: &Pubkey,
    dispute_key: &Pubkey,
    clock: &Clock,
) -> Result<u64> {
    for acc in remaining.iter() {
        if acc.owner == &crate::ID && acc.is_writable {
            let data = acc.try_borrow_data()?;
            if data.len() >= 8 && data[..8] == *AgentStake::DISCRIMINATOR {
                drop(data);

                // Deserialize properly via Anchor
                let mut stake: Account<AgentStake> = Account::try_from(acc)?;

                // C1 fix: Verify this stake belongs to the correct agent
                require!(stake.agent == *expected_agent, SapError::StakeAgentMismatch);

                // Calculate slash: SLASH_BPS / 10_000 of dispute amount
                let slash_amount = dispute_amount
                    .checked_mul(AgentStake::SLASH_BPS)
                    .ok_or(error!(SapError::ArithmeticOverflow))?
                    / 10_000;
                let actual_slash = slash_amount.min(stake.staked_amount);

                if actual_slash > 0 {
                    // Transfer lamports: stake → depositor
                    **acc.try_borrow_mut_lamports()? -= actual_slash;
                    **depositor.try_borrow_mut_lamports()? += actual_slash;

                    // Update ALL relevant fields (C1 fix)
                    stake.staked_amount = stake.staked_amount.saturating_sub(actual_slash);
                    stake.slashed_amount = stake.slashed_amount.saturating_add(actual_slash);
                    stake.total_disputes_lost = stake.total_disputes_lost.saturating_add(1);

                    // Serialize back
                    let mut data = acc.try_borrow_mut_data()?;
                    let mut writer = &mut data[8..]; // skip discriminator
                    stake.try_serialize(&mut writer)?;

                    // Emit event (C1 fix: was silently slashing)
                    emit!(StakeSlashedEvent {
                        agent: *expected_agent,
                        dispute: *dispute_key,
                        slash_amount: actual_slash,
                        remaining_staked: stake.staked_amount,
                        compensated_to: depositor.key(),
                        timestamp: clock.unix_timestamp,
                    });
                }

                return Ok(actual_slash);
            }
        }
    }

    Ok(0) // No stake account found — no slash
}
