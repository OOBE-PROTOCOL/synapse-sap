use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  DISPUTE RESOLUTION — Receipt-Based Auto-Resolution
//
//  v0.7 Flow (no arbiter):
//    1. Agent calls settle_calls_v2 (DisputeWindow mode)
//    2. Depositor reviews pending settlement
//    3. If contested → file_dispute(dispute_type, evidence_hash)
//    4. Agent submits receipt proofs via submit_receipt_proof()
//    5. auto_resolve_dispute() — permissionless resolution:
//       - All calls proven → AgentWins
//       - No proof + deadline passed → DepositorWins
//       - Partial proof → PartialRefund (proportional)
//       - Quality dispute → auto-checks then 50/50 fallback
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
    dispute_type: u8, // 0=NonDelivery, 1=PartialDelivery, 2=Overcharge, 3=Quality
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Must be within dispute window
    require!(
        current_slot < ctx.accounts.pending_settlement.release_slot,
        SapError::DisputeWindowExpired
    );

    // Parse dispute type
    let dtype = match dispute_type {
        0 => DisputeType::NonDelivery,
        1 => DisputeType::PartialDelivery,
        2 => DisputeType::Overcharge,
        3 => DisputeType::Quality,
        _ => return Err(error!(SapError::InvalidDisputeType)),
    };

    // Quality disputes require a bond (10% of disputed amount)
    let bond_amount = if dtype == DisputeType::Quality {
        let amount = ctx.accounts.pending_settlement.amount;
        amount
            .checked_mul(AgentStake::QUALITY_DISPUTE_BOND_BPS)
            .ok_or(error!(SapError::ArithmeticOverflow))?
            / 10_000
    } else {
        0
    };

    // v0.13: Track dispute bond in escrow state (orphan protection)
    if bond_amount > 0 {
        let escrow = &mut ctx.accounts.escrow;
        escrow.dispute_bond_total = escrow.dispute_bond_total
            .checked_add(bond_amount)
            .ok_or(error!(SapError::ArithmeticOverflow))?;
    }

    // Transfer bond if required
    if bond_amount > 0 {
        let depositor_info = ctx.accounts.depositor.to_account_info();
        let escrow_info = ctx.accounts.escrow.to_account_info();
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.key(),
                anchor_lang::system_program::Transfer {
                    from: depositor_info,
                    to: escrow_info,
                },
            ),
            bond_amount,
        )?;
    }

    let dispute = &mut ctx.accounts.dispute;
    dispute.bump = ctx.bumps.dispute;
    dispute.pending_settlement = ctx.accounts.pending_settlement.key();
    dispute.escrow = ctx.accounts.escrow.key();
    dispute.depositor = ctx.accounts.depositor.key();
    dispute.agent = ctx.accounts.escrow.agent;
    dispute.dispute_type = dtype;
    dispute.evidence_hash = evidence_hash;
    dispute.agent_evidence_hash = [0u8; 32];
    dispute.outcome = DisputeOutcome::Pending;
    dispute.resolution_layer = ResolutionLayer::Pending;
    dispute.resolution_hash = [0u8; 32];
    dispute.resolved_at = 0;
    dispute.created_at = clock.unix_timestamp;
    dispute.slash_amount = 0;
    dispute.dispute_bond = bond_amount;
    dispute.proven_calls = 0;
    dispute.claimed_calls = ctx.accounts.pending_settlement.calls_to_settle as u32;
    dispute.proof_deadline = clock.unix_timestamp + AgentStake::PROOF_DEADLINE_SECONDS;

    // Mark PendingSettlement as disputed so finalize_settlement is blocked
    ctx.accounts.pending_settlement.is_disputed = true;

    emit!(DisputeFiledEvent {
        dispute: dispute.key(),
        pending_settlement: ctx.accounts.pending_settlement.key(),
        escrow: ctx.accounts.escrow.key(),
        depositor: ctx.accounts.depositor.key(),
        agent: ctx.accounts.escrow.agent,
        evidence_hash,
        dispute_type,
        dispute_bond: bond_amount,
        proof_deadline: dispute.proof_deadline,
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
//  resolve_dispute — DEPRECATED in v0.7
//
//  Arbiter-based resolution replaced by auto_resolve_dispute
//  (receipt-based, permissionless).  Kept as no-op for IDL compat.
//  Use auto_resolve_dispute in receipt.rs instead.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct ResolveDisputeAccountConstraints<'info> {
    #[account(mut)]
    pub arbiter: Signer<'info>,

    /// CHECK: Depositor
    #[account(mut)]
    pub depositor: UncheckedAccount<'info>,

    /// CHECK: Agent wallet
    #[account(mut)]
    pub agent_wallet: UncheckedAccount<'info>,

    #[account(
        seeds = [b"sap_escrow_v2", escrow.agent.as_ref(), escrow.depositor.as_ref(), &escrow.escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    #[account(
        seeds = [b"sap_pending", escrow.key().as_ref(), &pending_settlement.settlement_index.to_le_bytes()],
        bump = pending_settlement.bump,
        constraint = pending_settlement.escrow == escrow.key(),
    )]
    pub pending_settlement: Account<'info, PendingSettlement>,

    #[account(
        seeds = [b"sap_dispute", pending_settlement.key().as_ref()],
        bump = dispute.bump,
    )]
    pub dispute: Account<'info, DisputeRecord>,

    #[account(
        seeds = [b"sap_stats", escrow.agent.as_ref()],
        bump = agent_stats.bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,
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
//  Internal: Slash from a typed AgentStake account (v0.11 H-3)
//
//  Was previously `try_slash_from_remaining` which scanned remaining_accounts
//  for an AccountInfo matching the AgentStake discriminator. That was unsafe
//  because if the caller omitted the stake the slash silently became a no-op.
//
//  v0.11 hardening: callers now pass an Option<&mut Account<AgentStake>>
//  obtained from a typed account in their context. The slash either runs or
//  the entire transaction fails — no silent skip.
//
//  Backwards-compatible no-arg legacy variant `try_slash_from_remaining` is
//  kept exported for any out-of-tree caller (none in this crate) but is
//  marked `#[deprecated]`.
// ══════════════════════════════════════════════════════════════════

/// v0.11 H-3: slash from a typed AgentStake account.
///
/// Returns the amount actually slashed (0 only if the agent had no stake left).
pub fn try_slash_from_account<'info>(
    stake: &mut Account<'info, AgentStake>,
    dispute_amount: u64,
    depositor: &AccountInfo<'info>,
    expected_agent: &Pubkey,
    dispute_key: &Pubkey,
    clock: &Clock,
) -> Result<u64> {
    // Defence in depth — the caller's #[account(seeds = ...)] already enforces
    // these, but check explicitly because a misconfigured caller is the only
    // way to reach this function with a mismatched stake.
    require!(stake.agent == *expected_agent, SapError::StakeAgentMismatch);

    let slash_amount = dispute_amount
        .checked_mul(AgentStake::SLASH_BPS)
        .ok_or(error!(SapError::ArithmeticOverflow))?
        / 10_000;
    let actual_slash = slash_amount.min(stake.staked_amount);

    if actual_slash == 0 {
        return Ok(0);
    }

    // Move lamports stake → depositor
    let stake_info = stake.to_account_info();
    **stake_info.try_borrow_mut_lamports()? -= actual_slash;
    **depositor.try_borrow_mut_lamports()? += actual_slash;

    // v0.13: handle pending unstake — slash reduces staked_amount, so if an
    // unstake is pending it must be reduced proportionally to prevent
    // `complete_unstake` from underflowing (staked_amount < unstake_amount).
    if stake.unstake_amount > 0 {
        let unstake_proportion = stake.unstake_amount
            .checked_mul(actual_slash)
            .ok_or(error!(SapError::ArithmeticOverflow))?
            / stake.staked_amount.max(1);
        stake.unstake_amount = stake.unstake_amount.saturating_sub(unstake_proportion);
        if stake.unstake_amount == 0 {
            stake.unstake_requested_at = 0;
            stake.unstake_available_at = 0;
        }
    }

    stake.staked_amount = stake.staked_amount.saturating_sub(actual_slash);
    stake.slashed_amount = stake.slashed_amount.saturating_add(actual_slash);
    stake.total_disputes_lost = stake.total_disputes_lost.saturating_add(1);

    emit!(StakeSlashedEvent {
        agent: *expected_agent,
        dispute: *dispute_key,
        slash_amount: actual_slash,
        remaining_staked: stake.staked_amount,
        compensated_to: depositor.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(actual_slash)
}

// ══════════════════════════════════════════════════════════════════
//  DEPRECATED: try_slash_from_remaining (pre-v0.11)
//
//  Left in place so any external caller still compiles, but every internal
//  caller has been migrated to `try_slash_from_account`. Will be removed in
//  v0.12 along with the active_obligations migration.
// ══════════════════════════════════════════════════════════════════

#[deprecated(note = "v0.11: use try_slash_from_account with a typed AgentStake account")]
pub fn try_slash_from_remaining<'info>(
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
