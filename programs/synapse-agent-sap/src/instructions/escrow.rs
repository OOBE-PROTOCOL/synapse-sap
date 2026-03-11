use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  x402 ESCROW SETTLEMENT LAYER — Native Solana Micropayments
//
//  Pre-funded trustless settlement between clients and agents.
//
//  Flow:
//    1. Client creates escrow with locked-in price + initial deposit
//    2. Client calls agent via x402 HTTP endpoint
//    3. Agent settles onchain → claims SOL, emits receipt in TX log
//    4. Client can withdraw remaining balance at any time
//    5. Close escrow when done (rent returned to depositor)
//
//  Settlement model: agent self-reports calls served (same trust
//  pattern as report_calls).  Each settlement includes a service_hash
//  (sha256 proof of work) for dispute resolution.
//
//  Onchain guarantees:
//    - Price per call is immutable after creation
//    - max_calls limits total exposure
//    - Client can always withdraw remaining balance
//    - PaymentSettledEvent = permanent zero-rent receipt
//    - settle_calls also increments agent.total_calls_served
//
//  Cost model: ~0.002 SOL for escrow PDA rent.
//  Settlement = direct lamport transfer (no CPI, no token accounts).
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  create_escrow — Pre-fund micropayments for an agent
//  Seeds: ["sap_escrow", agent_pda, depositor_wallet]
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CreateEscrowAccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    /// Agent to prepay for — anyone can create escrow
    #[account(
        constraint = agent.is_active @ SapError::AgentInactive,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init,
        payer = depositor,
        space = EscrowAccount::DISCRIMINATOR.len() + EscrowAccount::INIT_SPACE,
        seeds = [b"sap_escrow", agent.key().as_ref(), depositor.key().as_ref()],
        bump,
    )]
    pub escrow: Account<'info, EscrowAccount>,

    pub system_program: Program<'info, System>,
}

pub fn create_escrow_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, CreateEscrowAccountConstraints<'info>>,
    price_per_call: u64,
    max_calls: u64,
    initial_deposit: u64,
    expires_at: i64,
    volume_curve: Vec<VolumeCurveBreakpoint>,
    token_mint: Option<Pubkey>,
    token_decimals: u8,
) -> Result<()> {
    let clock = Clock::get()?;

    // Validate volume curve
    validate_volume_curve(&volume_curve)?;

    // Phase 1: Cache AccountInfos before mutable borrow
    let remaining = ctx.remaining_accounts;
    let depositor_info = ctx.accounts.depositor.to_account_info();
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let sys_info = ctx.accounts.system_program.to_account_info();
    let agent_key = ctx.accounts.agent.key();
    let agent_wallet = ctx.accounts.agent.wallet;
    let depositor_key = ctx.accounts.depositor.key();

    // Phase 2: Initialize escrow
    let escrow = &mut ctx.accounts.escrow;
    escrow.bump = ctx.bumps.escrow;
    escrow.agent = agent_key;
    escrow.depositor = depositor_key;
    escrow.agent_wallet = agent_wallet;
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

    // Phase 3: Transfer initial deposit (uses cached AccountInfos)
    if initial_deposit > 0 {
        if token_mint.is_some() {
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

    emit!(EscrowCreatedEvent {
        escrow: escrow.key(),
        agent: agent_key,
        depositor: depositor_key,
        price_per_call,
        max_calls,
        initial_deposit,
        expires_at,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  deposit_escrow — Add more SOL to an existing escrow
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct DepositEscrowAccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_escrow", escrow.agent.as_ref(), depositor.key().as_ref()],
        bump = escrow.bump,
        has_one = depositor,
    )]
    pub escrow: Account<'info, EscrowAccount>,

    pub system_program: Program<'info, System>,
}

pub fn deposit_escrow_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, DepositEscrowAccountConstraints<'info>>,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;

    // Phase 1: Cache before mutable borrow
    let remaining = ctx.remaining_accounts;
    let depositor_info = ctx.accounts.depositor.to_account_info();
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let sys_info = ctx.accounts.system_program.to_account_info();
    let is_spl = ctx.accounts.escrow.token_mint.is_some();
    let token_mint = ctx.accounts.escrow.token_mint;

    // Reject deposits to expired escrows
    if ctx.accounts.escrow.expires_at > 0 {
        require!(
            clock.unix_timestamp < ctx.accounts.escrow.expires_at,
            SapError::EscrowExpired
        );
    }

    // Phase 2: Transfer (cached AccountInfos)
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

    // Phase 3: Mutable state update
    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow.balance
        .checked_add(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.total_deposited = escrow.total_deposited
        .checked_add(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

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
//  settle_calls — Agent claims payment for calls served
//
//  Native Solana optimizations vs ERC-8004:
//    1. Agent PDA is NOT deserialized (UncheckedAccount, 0 alloc
//       vs 8 KB).  Only AgentStats (106 B) is loaded.
//    2. Volume curve enforcement — tiered pricing spans tier
//       boundaries.  Cumulative calls determine effective price.
//    3. SPL token support — raw CPI to Token Program, zero
//       additional crate dependencies.
//    4. No GlobalRegistry contention — concurrent settlements
//       across agents do not conflict.
//
//  PaymentSettledEvent = permanent receipt in TX log (zero rent).
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SettleCallsAccountConstraints<'info> {
    /// Agent owner — signs settlement + receives payment
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified, NOT deserialized.
    /// PDA derivation for escrow + stats.
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
        seeds = [b"sap_escrow", agent.key().as_ref(), escrow.depositor.as_ref()],
        bump = escrow.bump,
        constraint = escrow.agent == agent.key(),
    )]
    pub escrow: Account<'info, EscrowAccount>,
}

pub fn settle_calls_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, SettleCallsAccountConstraints<'info>>,
    calls_to_settle: u64,
    service_hash: [u8; 32],
) -> Result<()> {
    let clock = Clock::get()?;

    // Phase 1: Cache AccountInfos before mutable borrow
    let remaining = ctx.remaining_accounts;
    let wallet_info = ctx.accounts.wallet.to_account_info();
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let agent_key = ctx.accounts.agent.key();

    // Phase 2: Read-only validation (temporary immutable borrows)
    let is_spl = ctx.accounts.escrow.token_mint.is_some();
    let escrow_agent = ctx.accounts.escrow.agent;
    let escrow_depositor = ctx.accounts.escrow.depositor;
    let escrow_bump = ctx.accounts.escrow.bump;
    let token_mint = ctx.accounts.escrow.token_mint;

    require!(calls_to_settle >= 1, SapError::InvalidSettlementCalls);

    if ctx.accounts.escrow.expires_at > 0 {
        require!(
            clock.unix_timestamp < ctx.accounts.escrow.expires_at,
            SapError::EscrowExpired
        );
    }

    if ctx.accounts.escrow.max_calls > 0 {
        require!(
            ctx.accounts.escrow.total_calls_settled + calls_to_settle <= ctx.accounts.escrow.max_calls,
            SapError::EscrowMaxCallsExceeded
        );
    }

    // Volume curve aware pricing
    let amount = calculate_settle_amount(
        ctx.accounts.escrow.price_per_call,
        &ctx.accounts.escrow.volume_curve,
        ctx.accounts.escrow.total_calls_settled,
        calls_to_settle,
    )?;

    require!(ctx.accounts.escrow.balance >= amount, SapError::InsufficientEscrowBalance);

    // Phase 3: Transfer payment (using cached AccountInfos)
    if is_spl {
        spl_transfer_from_escrow(
            &escrow_info, remaining,
            &escrow_agent, &escrow_depositor, escrow_bump,
            amount, token_mint,
        )?;
    } else {
        **escrow_info.try_borrow_mut_lamports()? -= amount;
        **wallet_info.try_borrow_mut_lamports()? += amount;
    }

    // Phase 4: Mutable state updates
    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow.balance
        .checked_sub(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.total_settled = escrow.total_settled
        .checked_add(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.total_calls_settled = escrow.total_calls_settled
        .checked_add(calls_to_settle)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.last_settled_at = clock.unix_timestamp;

    let stats = &mut ctx.accounts.agent_stats;
    stats.total_calls_served = stats.total_calls_served
        .checked_add(calls_to_settle)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
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

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  withdraw_escrow — Client withdraws SOL from their escrow
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct WithdrawEscrowAccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sap_escrow", escrow.agent.as_ref(), depositor.key().as_ref()],
        bump = escrow.bump,
        has_one = depositor,
    )]
    pub escrow: Account<'info, EscrowAccount>,
}

pub fn withdraw_escrow_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, WithdrawEscrowAccountConstraints<'info>>,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;

    // Phase 1: Cache before mutable borrow
    let remaining = ctx.remaining_accounts;
    let depositor_info = ctx.accounts.depositor.to_account_info();
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let is_spl = ctx.accounts.escrow.token_mint.is_some();
    let escrow_agent = ctx.accounts.escrow.agent;
    let escrow_depositor = ctx.accounts.escrow.depositor;
    let escrow_bump = ctx.accounts.escrow.bump;
    let balance = ctx.accounts.escrow.balance;
    let token_mint = ctx.accounts.escrow.token_mint;

    require!(balance > 0, SapError::EscrowEmpty);

    let withdraw_amount = amount.min(balance);

    // Phase 2: Transfer (uses cached AccountInfos)
    if is_spl {
        spl_transfer_from_escrow(
            &escrow_info, remaining,
            &escrow_agent, &escrow_depositor, escrow_bump,
            withdraw_amount, token_mint,
        )?;
    } else {
        **escrow_info.try_borrow_mut_lamports()? -= withdraw_amount;
        **depositor_info.try_borrow_mut_lamports()? += withdraw_amount;
    }

    // Phase 3: Mutable state update
    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow.balance
        .checked_sub(withdraw_amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

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
//  close_escrow — Close an empty escrow PDA (rent returned)
//  Requires balance == 0 (withdraw first).
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseEscrowAccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        close = depositor,
        seeds = [b"sap_escrow", escrow.agent.as_ref(), depositor.key().as_ref()],
        bump = escrow.bump,
        has_one = depositor,
        constraint = escrow.balance == 0 @ SapError::EscrowNotEmpty,
    )]
    pub escrow: Account<'info, EscrowAccount>,
}

pub fn close_escrow_handler(_ctx: Context<CloseEscrowAccountConstraints>) -> Result<()> {
    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  settle_batch — Batch settlement (N settlements in 1 TX)
//
//  Reduces TX fees by ~N× vs individual settle_calls.
//  All settlements target the same escrow.
//
//  Max 10 settlements per batch to fit in compute budget.
//  Volume curve pricing spans across the entire batch.
//  One transfer for the total amount.
//  Each settlement's service_hash preserved in BatchSettledEvent.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SettleBatchAccountConstraints<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,

    /// CHECK: Agent PDA — seeds-verified, NOT deserialized.
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
        seeds = [b"sap_escrow", agent.key().as_ref(), escrow.depositor.as_ref()],
        bump = escrow.bump,
        constraint = escrow.agent == agent.key(),
    )]
    pub escrow: Account<'info, EscrowAccount>,
}

pub fn settle_batch_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, SettleBatchAccountConstraints<'info>>,
    settlements: Vec<Settlement>,
) -> Result<()> {
    require!(!settlements.is_empty(), SapError::BatchEmpty);
    require!(settlements.len() <= 10, SapError::BatchTooLarge);

    let clock = Clock::get()?;

    // Phase 1: Cache before mutable borrow
    let remaining = ctx.remaining_accounts;
    let wallet_info = ctx.accounts.wallet.to_account_info();
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let agent_key = ctx.accounts.agent.key();
    let is_spl = ctx.accounts.escrow.token_mint.is_some();
    let escrow_agent = ctx.accounts.escrow.agent;
    let escrow_depositor = ctx.accounts.escrow.depositor;
    let escrow_bump = ctx.accounts.escrow.bump;
    let token_mint = ctx.accounts.escrow.token_mint;

    // Phase 2: Validate and compute totals
    if ctx.accounts.escrow.expires_at > 0 {
        require!(
            clock.unix_timestamp < ctx.accounts.escrow.expires_at,
            SapError::EscrowExpired
        );
    }

    let mut total_calls: u64 = 0;
    let mut service_hashes: Vec<[u8; 32]> = Vec::with_capacity(settlements.len());
    let mut calls_list: Vec<u64> = Vec::with_capacity(settlements.len());

    for s in &settlements {
        require!(s.calls_to_settle >= 1, SapError::InvalidSettlementCalls);
        total_calls = total_calls
            .checked_add(s.calls_to_settle)
            .ok_or(error!(SapError::ArithmeticOverflow))?;
        service_hashes.push(s.service_hash);
        calls_list.push(s.calls_to_settle);
    }

    if ctx.accounts.escrow.max_calls > 0 {
        require!(
            ctx.accounts.escrow.total_calls_settled + total_calls <= ctx.accounts.escrow.max_calls,
            SapError::EscrowMaxCallsExceeded
        );
    }

    let total_amount = calculate_settle_amount(
        ctx.accounts.escrow.price_per_call,
        &ctx.accounts.escrow.volume_curve,
        ctx.accounts.escrow.total_calls_settled,
        total_calls,
    )?;

    require!(ctx.accounts.escrow.balance >= total_amount, SapError::InsufficientEscrowBalance);

    // Phase 3: Transfer
    if is_spl {
        spl_transfer_from_escrow(
            &escrow_info, remaining,
            &escrow_agent, &escrow_depositor, escrow_bump,
            total_amount, token_mint,
        )?;
    } else {
        **escrow_info.try_borrow_mut_lamports()? -= total_amount;
        **wallet_info.try_borrow_mut_lamports()? += total_amount;
    }

    // Phase 4: Mutable state updates
    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow.balance
        .checked_sub(total_amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.total_settled = escrow.total_settled
        .checked_add(total_amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.total_calls_settled = escrow.total_calls_settled
        .checked_add(total_calls)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.last_settled_at = clock.unix_timestamp;

    let stats = &mut ctx.accounts.agent_stats;
    stats.total_calls_served = stats.total_calls_served
        .checked_add(total_calls)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    stats.updated_at = clock.unix_timestamp;

    emit!(BatchSettledEvent {
        escrow: escrow.key(),
        agent: agent_key,
        depositor: escrow.depositor,
        num_settlements: settlements.len() as u8,
        total_calls,
        total_amount,
        service_hashes,
        calls_per_settlement: calls_list,
        remaining_balance: escrow.balance,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Helper Functions — Volume Curve & SPL Token Support
// ═══════════════════════════════════════════════════════════════════

/// Validate volume curve breakpoints (max 5, ascending after_calls).
fn validate_volume_curve(curve: &[VolumeCurveBreakpoint]) -> Result<()> {
    require!(curve.len() <= EscrowAccount::MAX_VOLUME_CURVE, SapError::TooManyVolumeCurvePoints);
    let mut prev = 0u32;
    for bp in curve {
        require!(bp.after_calls > prev || prev == 0, SapError::InvalidVolumeCurve);
        prev = bp.after_calls;
    }
    Ok(())
}

/// Tiered settlement: spans tier boundaries. Cumulative calls determine price.
fn calculate_settle_amount(
    base_price: u64,
    curve: &[VolumeCurveBreakpoint],
    total_before: u64,
    calls: u64,
) -> Result<u64> {
    if curve.is_empty() {
        return calls
            .checked_mul(base_price)
            .ok_or(error!(SapError::ArithmeticOverflow));
    }

    let mut amount: u64 = 0;
    let mut remaining = calls;
    let mut cursor = total_before;

    while remaining > 0 {
        // Find current price at cursor position
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

        // How many calls at this price level?
        let calls_at_price = if let Some(threshold) = next_threshold {
            let to_threshold = threshold.saturating_sub(cursor);
            remaining.min(to_threshold)
        } else {
            remaining
        };

        amount = amount
            .checked_add(
                calls_at_price
                    .checked_mul(current_price)
                    .ok_or(error!(SapError::ArithmeticOverflow))?,
            )
            .ok_or(error!(SapError::ArithmeticOverflow))?;

        remaining -= calls_at_price;
        cursor += calls_at_price;
    }

    Ok(amount)
}

/// SPL Token Program ID: TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
fn is_spl_token_program(key: &Pubkey) -> bool {
    key.as_ref() == [
        6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70,
        206, 235, 121, 172, 28, 180, 133, 237, 95, 91, 55, 145,
        58, 140, 245, 133, 126, 255, 0, 169,
    ]
}

/// Raw SPL Token Transfer CPI — zero additional crate dependencies.
/// Constructs the transfer instruction manually (opcode = 3, 9 bytes).
/// The escrow PDA signs via invoke_signed.
fn spl_transfer_from_escrow<'info>(
    escrow_info: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
    agent_key: &Pubkey,
    depositor_key: &Pubkey,
    escrow_bump: u8,
    amount: u64,
    expected_mint: Option<Pubkey>,
) -> Result<()> {
    require!(remaining.len() >= 3, SapError::SplTokenRequired);

    let escrow_token = &remaining[0];
    let dest_token = &remaining[1];
    let token_program = &remaining[2];

    // Validate source token account mint matches escrow.token_mint
    if let Some(mint) = expected_mint {
        let data = escrow_token.try_borrow_data()?;
        require!(data.len() >= 32, SapError::InvalidTokenAccount);
        let source_mint = Pubkey::try_from(&data[..32]).map_err(|_| error!(SapError::InvalidTokenAccount))?;
        require!(source_mint == mint, SapError::InvalidTokenAccount);
    }

    require!(is_spl_token_program(token_program.key), SapError::InvalidTokenProgram);

    // Build raw SPL Transfer instruction (opcode = 3)
    let mut data = Vec::with_capacity(9);
    data.push(3u8);
    data.extend_from_slice(&amount.to_le_bytes());

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
        b"sap_escrow",
        agent_key.as_ref(),
        depositor_key.as_ref(),
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

/// Raw SPL Token Transfer CPI — depositor/signer as authority.
/// Used for create_escrow + deposit_escrow with SPL tokens.
fn spl_transfer_from_signer<'info>(
    signer_info: &AccountInfo<'info>,
    remaining: &[AccountInfo<'info>],
    amount: u64,
    expected_mint: Option<Pubkey>,
) -> Result<()> {
    require!(remaining.len() >= 3, SapError::SplTokenRequired);

    let source_token = &remaining[0];
    let dest_token = &remaining[1];
    let token_program = &remaining[2];

    // Validate source token account mint matches expected
    if let Some(mint) = expected_mint {
        let data = source_token.try_borrow_data()?;
        require!(data.len() >= 32, SapError::InvalidTokenAccount);
        let source_mint = Pubkey::try_from(&data[..32]).map_err(|_| error!(SapError::InvalidTokenAccount))?;
        require!(source_mint == mint, SapError::InvalidTokenAccount);
    }

    require!(is_spl_token_program(token_program.key), SapError::InvalidTokenProgram);

    let mut data = Vec::with_capacity(9);
    data.push(3u8);
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: *token_program.key,
        accounts: vec![
            AccountMeta::new(*source_token.key, false),
            AccountMeta::new(*dest_token.key, false),
            AccountMeta::new_readonly(*signer_info.key, true),
        ],
        data,
    };

    invoke(
        &ix,
        &[
            source_token.clone(),
            dest_token.clone(),
            signer_info.clone(),
            token_program.clone(),
        ],
    )?;

    Ok(())
}
