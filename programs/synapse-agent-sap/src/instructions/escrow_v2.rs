use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  x402 ESCROW V2 — Triple-Mode Settlement Layer
//
//  Three settlement security models in one PDA:
//
//    1. SelfReport   — Agent settles unilaterally (v1 compatible)
//    2. CoSigned     — Agent + client co-sign every settlement
//    3. DisputeWindow — Settlement enters pending state,
//                       depositor can dispute within N slots
//
//  New features vs v1:
//    - escrow_nonce: multiple escrows per (agent, depositor) pair
//    - co_signer: bilateral settlement authorization
//    - arbiter: on-chain dispute resolution authority
//    - pending settlements: time-locked settlement with auto-release
//
//  All modes emit permanent TX log receipts.  Dispute mode also
//  creates PendingSettlement PDAs for granular tracking.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  create_escrow_v2
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_nonce: u64)]
pub struct CreateEscrowV2AccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        constraint = agent.is_active @ SapError::AgentInactive,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init,
        payer = depositor,
        space = EscrowAccountV2::DISCRIMINATOR.len() + EscrowAccountV2::INIT_SPACE,
        seeds = [b"sap_escrow_v2", agent.key().as_ref(), depositor.key().as_ref(), &escrow_nonce.to_le_bytes()],
        bump,
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    pub system_program: Program<'info, System>,
}

pub fn create_escrow_v2_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, CreateEscrowV2AccountConstraints<'info>>,
    escrow_nonce: u64,
    price_per_call: u64,
    max_calls: u64,
    initial_deposit: u64,
    expires_at: i64,
    volume_curve: Vec<VolumeCurveBreakpoint>,
    token_mint: Option<Pubkey>,
    token_decimals: u8,
    settlement_security: u8, // 0=SelfReport, 1=CoSigned, 2=DisputeWindow
    dispute_window_slots: u64,
    co_signer: Option<Pubkey>,
    arbiter: Option<Pubkey>,
) -> Result<()> {
    let clock = Clock::get()?;

    // Validate settlement security mode
    let security = match settlement_security {
        0 => SettlementSecurity::SelfReport,
        1 => {
            require!(co_signer.is_some(), SapError::CoSignerRequired);
            SettlementSecurity::CoSigned
        }
        2 => {
            require!(arbiter.is_some(), SapError::ArbiterRequired);
            require!(dispute_window_slots > 0, SapError::InvalidSettlementSecurity);
            SettlementSecurity::DisputeWindow
        }
        _ => return Err(error!(SapError::InvalidSettlementSecurity)),
    };

    // Validate volume curve
    require!(volume_curve.len() <= EscrowAccountV2::MAX_VOLUME_CURVE, SapError::TooManyVolumeCurvePoints);
    let mut prev = 0u32;
    for bp in &volume_curve {
        require!(bp.after_calls > prev || prev == 0, SapError::InvalidVolumeCurve);
        prev = bp.after_calls;
    }

    // Cache before mutable borrow
    let depositor_info = ctx.accounts.depositor.to_account_info();
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let sys_info = ctx.accounts.system_program.to_account_info();
    let agent_key = ctx.accounts.agent.key();
    let agent_wallet = ctx.accounts.agent.wallet;
    let depositor_key = ctx.accounts.depositor.key();

    // Initialize escrow
    let escrow = &mut ctx.accounts.escrow;
    escrow.bump = ctx.bumps.escrow;
    escrow.version = EscrowAccountV2::VERSION;
    escrow.agent = agent_key;
    escrow.depositor = depositor_key;
    escrow.agent_wallet = agent_wallet;
    escrow.escrow_nonce = escrow_nonce;
    escrow.balance = 0;
    escrow.total_deposited = 0;
    escrow.total_settled = 0;
    escrow.total_calls_settled = 0;
    escrow.price_per_call = price_per_call;
    escrow.max_calls = max_calls;
    escrow.created_at = clock.unix_timestamp;
    escrow.last_settled_at = 0;
    escrow.expires_at = expires_at;
    escrow.volume_curve = volume_curve;
    escrow.token_mint = token_mint;
    escrow.token_decimals = token_decimals;
    escrow.settlement_security = security;
    escrow.dispute_window_slots = dispute_window_slots;
    escrow.settlement_index = 0;
    escrow.co_signer = co_signer;
    escrow.arbiter = arbiter;
    escrow.pending_amount = 0;
    escrow.pending_calls = 0;

    // Transfer initial deposit
    if initial_deposit > 0 {
        if token_mint.is_some() {
            let remaining = ctx.remaining_accounts;
            spl_transfer_from_signer(&depositor_info, remaining, initial_deposit, token_mint)?;
        } else {
            system_program::transfer(
                CpiContext::new(
                    sys_info,
                    system_program::Transfer {
                        from: depositor_info,
                        to: escrow_info,
                    },
                ),
                initial_deposit,
            )?;
        }
        escrow.balance = initial_deposit;
        escrow.total_deposited = initial_deposit;
    }

    emit!(EscrowV2CreatedEvent {
        escrow: escrow.key(),
        agent: agent_key,
        depositor: depositor_key,
        escrow_nonce,
        price_per_call,
        max_calls,
        initial_deposit,
        settlement_security,
        dispute_window_slots,
        co_signer,
        arbiter,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  deposit_escrow_v2
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_nonce: u64)]
pub struct DepositEscrowV2AccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_escrow_v2", escrow.agent.as_ref(), depositor.key().as_ref(), &escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
        has_one = depositor,
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    pub system_program: Program<'info, System>,
}

pub fn deposit_escrow_v2_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, DepositEscrowV2AccountConstraints<'info>>,
    _escrow_nonce: u64,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;

    let remaining = ctx.remaining_accounts;
    let depositor_info = ctx.accounts.depositor.to_account_info();
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let sys_info = ctx.accounts.system_program.to_account_info();
    let is_spl = ctx.accounts.escrow.token_mint.is_some();
    let token_mint = ctx.accounts.escrow.token_mint;

    if ctx.accounts.escrow.expires_at > 0 {
        require!(clock.unix_timestamp < ctx.accounts.escrow.expires_at, SapError::EscrowExpired);
    }

    if is_spl {
        spl_transfer_from_signer(&depositor_info, remaining, amount, token_mint)?;
    } else {
        system_program::transfer(
            CpiContext::new(
                sys_info,
                system_program::Transfer {
                    from: depositor_info,
                    to: escrow_info,
                },
            ),
            amount,
        )?;
    }

    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow.balance.checked_add(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.total_deposited = escrow.total_deposited.checked_add(amount).ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(EscrowDepositedEvent {
        escrow: escrow.key(),
        depositor: ctx.accounts.depositor.key(),
        amount,
        new_balance: escrow.balance,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  settle_calls_v2 — Multi-mode settlement
//
//  Dispatches to the correct settlement flow based on
//  escrow.settlement_security:
//
//    SelfReport:    Immediate transfer (v1 compatible)
//    CoSigned:      Requires co_signer signature, immediate transfer
//    DisputeWindow: Creates PendingSettlement PDA, holds funds
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(escrow_nonce: u64)]
pub struct SettleCallsV2AccountConstraints<'info> {
    /// Agent owner signs
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified, NOT deserialized (76× savings).
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
    )]
    pub agent: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"sap_stats", agent.key().as_ref()],
        bump = agent_stats.bump,
        constraint = agent_stats.is_active @ SapError::AgentInactive,
    )]
    pub agent_stats: Account<'info, AgentStats>,

    #[account(
        mut,
        seeds = [b"sap_escrow_v2", agent.key().as_ref(), escrow.depositor.as_ref(), &escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
        constraint = escrow.agent == agent.key(),
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    pub system_program: Program<'info, System>,
}

pub fn settle_calls_v2_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, SettleCallsV2AccountConstraints<'info>>,
    _escrow_nonce: u64,
    calls_to_settle: u64,
    service_hash: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;

    require!(calls_to_settle >= 1, SapError::InvalidSettlementCalls);

    if ctx.accounts.escrow.expires_at > 0 {
        require!(clock.unix_timestamp < ctx.accounts.escrow.expires_at, SapError::EscrowExpired);
    }

    if ctx.accounts.escrow.max_calls > 0 {
        require!(
            ctx.accounts.escrow.total_calls_settled
                + ctx.accounts.escrow.pending_calls
                + calls_to_settle
                <= ctx.accounts.escrow.max_calls,
            SapError::EscrowMaxCallsExceeded
        );
    }

    // Calculate amount via volume curve
    let amount = calculate_settle_amount(
        ctx.accounts.escrow.price_per_call,
        &ctx.accounts.escrow.volume_curve,
        ctx.accounts.escrow.total_calls_settled + ctx.accounts.escrow.pending_calls,
        calls_to_settle,
    )?;

    let available_balance = ctx.accounts.escrow.balance
        .checked_sub(ctx.accounts.escrow.pending_amount)
        .ok_or(error!(SapError::InsufficientEscrowBalance))?;
    require!(available_balance >= amount, SapError::InsufficientEscrowBalance);

    match ctx.accounts.escrow.settlement_security {
        SettlementSecurity::SelfReport => {
            // Immediate transfer — v1 compatible
            let wallet_info = ctx.accounts.wallet.to_account_info();
            let escrow_info = ctx.accounts.escrow.to_account_info();
            let remaining = ctx.remaining_accounts;
            let agent_key = ctx.accounts.agent.key();

            if ctx.accounts.escrow.token_mint.is_some() {
                spl_transfer_from_escrow_v2(
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
            escrow.total_calls_settled = escrow.total_calls_settled.checked_add(calls_to_settle).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.last_settled_at = clock.unix_timestamp;

            let stats = &mut ctx.accounts.agent_stats;
            stats.total_calls_served = stats.total_calls_served.checked_add(calls_to_settle).ok_or(error!(SapError::ArithmeticOverflow))?;
            stats.updated_at = clock.unix_timestamp;

            emit!(PaymentSettledEvent {
                escrow: escrow.key(),
                agent: agent_key,
                depositor: escrow.depositor,
                calls_settled: calls_to_settle,
                amount,
                service_hash,
                total_calls_settled: escrow.total_calls_settled,
                remaining_balance: escrow.balance,
                timestamp: clock.unix_timestamp,
            });
        }
        SettlementSecurity::CoSigned => {
            // Verify co-signer is present in remaining accounts as a Signer
            let co_signer = ctx.accounts.escrow.co_signer.ok_or(error!(SapError::CoSignerRequired))?;
            let remaining = ctx.remaining_accounts;
            let mut co_signed = false;
            for acc in remaining.iter() {
                if acc.key() == co_signer && acc.is_signer {
                    co_signed = true;
                    break;
                }
            }
            require!(co_signed, SapError::InvalidCoSigner);

            // Immediate transfer with co-signature verification
            let wallet_info = ctx.accounts.wallet.to_account_info();
            let escrow_info = ctx.accounts.escrow.to_account_info();
            let agent_key = ctx.accounts.agent.key();

            if ctx.accounts.escrow.token_mint.is_some() {
                spl_transfer_from_escrow_v2(
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
            escrow.total_calls_settled = escrow.total_calls_settled.checked_add(calls_to_settle).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.last_settled_at = clock.unix_timestamp;

            let stats = &mut ctx.accounts.agent_stats;
            stats.total_calls_served = stats.total_calls_served.checked_add(calls_to_settle).ok_or(error!(SapError::ArithmeticOverflow))?;
            stats.updated_at = clock.unix_timestamp;

            emit!(CoSignedSettlementEvent {
                escrow: escrow.key(),
                agent: agent_key,
                depositor: escrow.depositor,
                co_signer,
                calls_settled: calls_to_settle,
                amount,
                service_hash,
                timestamp: clock.unix_timestamp,
            });
        }
        SettlementSecurity::DisputeWindow => {
            // Lock funds in pending — no immediate transfer
            let escrow = &mut ctx.accounts.escrow;
            let settlement_index = escrow.settlement_index;
            escrow.settlement_index = escrow.settlement_index.checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.pending_amount = escrow.pending_amount.checked_add(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
            escrow.pending_calls = escrow.pending_calls.checked_add(calls_to_settle).ok_or(error!(SapError::ArithmeticOverflow))?;

            let current_slot = Clock::get()?.slot;
            let release_slot = current_slot.checked_add(escrow.dispute_window_slots).ok_or(error!(SapError::ArithmeticOverflow))?;

            // M2 fix: Compute PendingSettlement PDA for the event
            let (pending_pda, _) = Pubkey::find_program_address(
                &[b"sap_pending", escrow.key().as_ref(), &settlement_index.to_le_bytes()],
                &crate::ID,
            );

            emit!(SettlementPendingEvent {
                pending_settlement: pending_pda,
                escrow: escrow.key(),
                agent: ctx.accounts.agent.key(),
                depositor: escrow.depositor,
                settlement_index,
                calls_to_settle,
                amount,
                service_hash,
                release_slot,
                timestamp: clock.unix_timestamp,
            });
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  create_pending_settlement — Create PendingSettlement PDA
//  Called in same TX as settle_calls_v2 (DisputeWindow mode)
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(settlement_index: u64)]
pub struct CreatePendingSettlementAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified.
    #[account(
        seeds = [b"sap_agent", wallet.key().as_ref()],
        bump,
    )]
    pub agent: UncheckedAccount<'info>,

    #[account(
        seeds = [b"sap_escrow_v2", agent.key().as_ref(), escrow.depositor.as_ref(), &escrow.escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
        constraint = escrow.agent == agent.key(),
        constraint = escrow.settlement_security == SettlementSecurity::DisputeWindow @ SapError::InvalidSettlementSecurity,
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    #[account(
        init,
        payer = wallet,
        space = PendingSettlement::DISCRIMINATOR.len() + PendingSettlement::INIT_SPACE,
        seeds = [b"sap_pending", escrow.key().as_ref(), &settlement_index.to_le_bytes()],
        bump,
    )]
    pub pending_settlement: Account<'info, PendingSettlement>,

    pub system_program: Program<'info, System>,
}

pub fn create_pending_settlement_handler(
    ctx: Context<CreatePendingSettlementAccountConstraints>,
    settlement_index: u64,
    calls_to_settle: u64,
    amount: u64,
    service_hash: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    let release_slot = current_slot
        .checked_add(ctx.accounts.escrow.dispute_window_slots)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    let ps = &mut ctx.accounts.pending_settlement;
    ps.bump = ctx.bumps.pending_settlement;
    ps.escrow = ctx.accounts.escrow.key();
    ps.agent = ctx.accounts.agent.key();
    ps.agent_wallet = ctx.accounts.escrow.agent_wallet;
    ps.depositor = ctx.accounts.escrow.depositor;
    ps.settlement_index = settlement_index;
    ps.calls_to_settle = calls_to_settle;
    ps.amount = amount;
    ps.service_hash = service_hash;
    ps.created_at = clock.unix_timestamp;
    ps.release_slot = release_slot;
    ps.is_finalized = false;
    ps.is_disputed = false;
    ps.outcome = DisputeOutcome::Pending;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  finalize_settlement — Release pending funds after dispute window
//
//  Anyone can call this — it's permissionless cranking.
//  Verifies that the dispute window has passed and no dispute
//  was filed, then transfers funds to the agent.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct FinalizeSettlementAccountConstraints<'info> {
    /// Anyone can crank
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Agent wallet — receives payment
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
        constraint = !pending_settlement.is_disputed @ SapError::SettlementDisputed,
        constraint = pending_settlement.outcome == DisputeOutcome::Pending @ SapError::SettlementNotPending,
    )]
    pub pending_settlement: Account<'info, PendingSettlement>,

    #[account(
        mut,
        seeds = [b"sap_stats", escrow.agent.as_ref()],
        bump = agent_stats.bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,
}

pub fn finalize_settlement_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, FinalizeSettlementAccountConstraints<'info>>,
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Dispute window must have passed
    require!(
        current_slot >= ctx.accounts.pending_settlement.release_slot,
        SapError::DisputeWindowNotExpired
    );

    // Verify agent_wallet matches
    require!(
        ctx.accounts.agent_wallet.key() == ctx.accounts.pending_settlement.agent_wallet,
        SapError::InvalidAgentWallet
    );

    let amount = ctx.accounts.pending_settlement.amount;
    let calls = ctx.accounts.pending_settlement.calls_to_settle;

    // Transfer funds
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let wallet_info = ctx.accounts.agent_wallet.to_account_info();

    if ctx.accounts.escrow.token_mint.is_some() {
        let remaining = ctx.remaining_accounts;
        spl_transfer_from_escrow_v2(
            &escrow_info, remaining,
            &ctx.accounts.escrow.agent, &ctx.accounts.escrow.depositor,
            ctx.accounts.escrow.escrow_nonce, ctx.accounts.escrow.bump,
            amount, ctx.accounts.escrow.token_mint,
        )?;
    } else {
        **escrow_info.try_borrow_mut_lamports()? -= amount;
        **wallet_info.try_borrow_mut_lamports()? += amount;
    }

    // Update escrow
    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow.balance.checked_sub(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.total_settled = escrow.total_settled.checked_add(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.total_calls_settled = escrow.total_calls_settled.checked_add(calls).ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.pending_amount = escrow.pending_amount.checked_sub(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.pending_calls = escrow.pending_calls.checked_sub(calls).ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.last_settled_at = clock.unix_timestamp;

    // Update agent stats
    let stats = &mut ctx.accounts.agent_stats;
    stats.total_calls_served = stats.total_calls_served.checked_add(calls).ok_or(error!(SapError::ArithmeticOverflow))?;
    stats.updated_at = clock.unix_timestamp;

    // Finalize pending settlement
    let ps = &mut ctx.accounts.pending_settlement;
    ps.is_finalized = true;
    ps.outcome = DisputeOutcome::AutoReleased;

    emit!(SettlementFinalizedEvent {
        pending_settlement: ps.key(),
        escrow: escrow.key(),
        agent: escrow.agent,
        amount,
        calls_settled: calls,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  withdraw_escrow_v2
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct WithdrawEscrowV2AccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_escrow_v2", escrow.agent.as_ref(), depositor.key().as_ref(), &escrow.escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
        has_one = depositor,
    )]
    pub escrow: Account<'info, EscrowAccountV2>,
}

pub fn withdraw_escrow_v2_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, WithdrawEscrowV2AccountConstraints<'info>>,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;

    // Available = balance - pending locked amount
    let available = ctx.accounts.escrow.balance
        .checked_sub(ctx.accounts.escrow.pending_amount)
        .ok_or(error!(SapError::InsufficientEscrowBalance))?;
    require!(available > 0, SapError::EscrowEmpty);

    let withdraw_amount = amount.min(available);

    let depositor_info = ctx.accounts.depositor.to_account_info();
    let escrow_info = ctx.accounts.escrow.to_account_info();

    if ctx.accounts.escrow.token_mint.is_some() {
        let remaining = ctx.remaining_accounts;
        spl_transfer_from_escrow_v2(
            &escrow_info, remaining,
            &ctx.accounts.escrow.agent, &ctx.accounts.escrow.depositor,
            ctx.accounts.escrow.escrow_nonce, ctx.accounts.escrow.bump,
            withdraw_amount, ctx.accounts.escrow.token_mint,
        )?;
    } else {
        **escrow_info.try_borrow_mut_lamports()? -= withdraw_amount;
        **depositor_info.try_borrow_mut_lamports()? += withdraw_amount;
    }

    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow.balance.checked_sub(withdraw_amount).ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(EscrowWithdrawnEvent {
        escrow: escrow.key(),
        depositor: ctx.accounts.depositor.key(),
        amount: withdraw_amount,
        remaining_balance: escrow.balance,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_escrow_v2
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseEscrowV2AccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        close = depositor,
        seeds = [b"sap_escrow_v2", escrow.agent.as_ref(), depositor.key().as_ref(), &escrow.escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
        has_one = depositor,
        constraint = escrow.balance == 0 @ SapError::EscrowNotEmpty,
        constraint = escrow.pending_amount == 0 @ SapError::SettlementNotPending,
    )]
    pub escrow: Account<'info, EscrowAccountV2>,
}

pub fn close_escrow_v2_handler(ctx: Context<CloseEscrowV2AccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;
    let escrow = &ctx.accounts.escrow;

    emit!(EscrowClosedEvent {
        escrow: escrow.key(),
        agent: escrow.agent,
        depositor: escrow.depositor,
        total_settled: escrow.total_settled,
        total_calls_settled: escrow.total_calls_settled,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Helper: Volume Curve Settlement Calculation (shared with v1)
// ═══════════════════════════════════════════════════════════════════

pub fn calculate_settle_amount(
    base_price: u64,
    curve: &[VolumeCurveBreakpoint],
    total_before: u64,
    calls: u64,
) -> Result<u64> {
    if curve.is_empty() {
        return calls.checked_mul(base_price).ok_or(error!(SapError::ArithmeticOverflow));
    }

    let mut amount: u64 = 0;
    let mut remaining = calls;
    let mut cursor = total_before;

    while remaining > 0 {
        let mut current_price = base_price;
        let mut next_threshold: Option<u64> = None;

        for bp in curve {
            let threshold = bp.after_calls as u64;
            if cursor >= threshold {
                current_price = bp.price_per_call;
            } else {
                next_threshold = Some(threshold);
                break;
            }
        }

        let calls_at_price = if let Some(threshold) = next_threshold {
            remaining.min(threshold.saturating_sub(cursor))
        } else {
            remaining
        };

        amount = amount.checked_add(
            calls_at_price.checked_mul(current_price).ok_or(error!(SapError::ArithmeticOverflow))?,
        ).ok_or(error!(SapError::ArithmeticOverflow))?;

        remaining -= calls_at_price;
        cursor += calls_at_price;
    }

    Ok(amount)
}

// ═══════════════════════════════════════════════════════════════════
//  Helper: SPL Transfer from EscrowV2 PDA (v2 seeds)
// ═══════════════════════════════════════════════════════════════════

fn spl_transfer_from_signer<'info>(
    depositor_info: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
    amount: u64,
    expected_mint: Option<Pubkey>,
) -> Result<()> {
    require!(remaining.len() >= 3, SapError::SplTokenRequired);

    let source_token = &remaining[0];
    let dest_token = &remaining[1];
    let token_program = &remaining[2];

    if let Some(mint) = expected_mint {
        let data = source_token.try_borrow_data()?;
        require!(data.len() >= 32, SapError::InvalidTokenAccount);
        let source_mint = Pubkey::try_from(&data[..32]).map_err(|_| error!(SapError::InvalidTokenAccount))?;
        require!(source_mint == mint, SapError::InvalidTokenAccount);
    }

    // M1-NEW fix: Validate token program ID (matches v1 security)
    require!(super::escrow::is_spl_token_program(token_program.key), SapError::InvalidTokenProgram);

    let mut data = Vec::with_capacity(9);
    data.push(3u8);
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: *token_program.key,
        accounts: vec![
            AccountMeta::new(*source_token.key, false),
            AccountMeta::new(*dest_token.key, false),
            AccountMeta::new_readonly(*depositor_info.key, true),
        ],
        data,
    };

    anchor_lang::solana_program::program::invoke(
        &ix,
        &[
            source_token.clone(),
            dest_token.clone(),
            depositor_info.clone(),
            token_program.clone(),
        ],
    )?;

    Ok(())
}

pub fn spl_transfer_from_escrow_v2<'info>(
    escrow_info: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
    agent_key: &Pubkey,
    depositor_key: &Pubkey,
    escrow_nonce: u64,
    escrow_bump: u8,
    amount: u64,
    expected_mint: Option<Pubkey>,
) -> Result<()> {
    require!(remaining.len() >= 3, SapError::SplTokenRequired);

    let escrow_token = &remaining[0];
    let dest_token = &remaining[1];
    let token_program = &remaining[2];

    if let Some(mint) = expected_mint {
        let data = escrow_token.try_borrow_data()?;
        require!(data.len() >= 32, SapError::InvalidTokenAccount);
        let source_mint = Pubkey::try_from(&data[..32]).map_err(|_| error!(SapError::InvalidTokenAccount))?;
        require!(source_mint == mint, SapError::InvalidTokenAccount);
    }

    // M1-NEW fix: Validate token program ID (matches v1 security)
    require!(super::escrow::is_spl_token_program(token_program.key), SapError::InvalidTokenProgram);

    let mut data = Vec::with_capacity(9);
    data.push(3u8);
    data.extend_from_slice(&amount.to_le_bytes());

    let nonce_bytes = escrow_nonce.to_le_bytes();
    let ix = Instruction {
        program_id: *token_program.key,
        accounts: vec![
            AccountMeta::new(*escrow_token.key, false),
            AccountMeta::new(*dest_token.key, false),
            AccountMeta::new_readonly(*escrow_info.key, true),
        ],
        data,
    };

    let seeds: &[&[u8]] = &[
        b"sap_escrow_v2",
        agent_key.as_ref(),
        depositor_key.as_ref(),
        &nonce_bytes,
        &[escrow_bump],
    ];

    invoke_signed(
        &ix,
        &[
            escrow_token.clone(),
            dest_token.clone(),
            escrow_info.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;

    Ok(())
}
