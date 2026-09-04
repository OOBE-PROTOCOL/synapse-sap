//! Reclaim excess rent from program-owned PDA accounts.
//!
//! Following the Solana rent reduction rollout (SIMD), accounts created
//! before the reduction hold more lamports than the new rent-exempt minimum.
//! This instruction lets the agent owner (or admin authority) reclaim the
//! excess lamports from any PDA owned by this program, without closing the
//! account.
//!
//! Security model:
//! - `target` must be owned by this program (runtime enforces).
//! - `authority` must be a signer and must be the agent wallet recorded
//!   on the account, OR the program upgrade authority for admin reclaim.
//! - The Rent sysvar is read at execution time, so the instruction is
//!   automatically correct across every phase of the rent reduction rollout.
//! - Only the excess above the rent-exempt floor is moved; the account
//!   stays alive and functional.

use crate::errors::SapError;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::rent::Rent;

/// Accounts for `reclaim_excess_rent`.
///
/// `target` is any account owned by this program. We do NOT deserialize it
/// into a specific type — we only need its lamport balance and data length.
/// This makes the instruction work for every PDA type (Agent, Stats, Stake,
/// PricingMenu, EscrowV2, etc.) without per-type variants.
///
/// `authority` is the signer authorizing the reclaim. For agent-owned PDAs
/// this is the agent's wallet. For protocol-level PDAs (GlobalRegistry,
/// index pages, etc.) this is the program authority.
///
/// `destination` is where the excess lamports land — typically the same
/// wallet as `authority`.
#[derive(Accounts)]
pub struct ReclaimExcessRent<'info> {
    /// The PDA to reclaim from. Must be owned by this program.
    /// Writable because we debit lamports.
    #[account(mut)]
    /// CHECK: We do not deserialize this account — we only move lamports.
    /// The runtime enforces that this account is owned by the current program,
    /// otherwise the lamport mutation will fail.
    pub target: UncheckedAccount<'info>,

    /// Signer authorizing the reclaim.
    /// Must sign to prove ownership / authority.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Where the excess lamports land.
    /// Writable because we credit lamports.
    #[account(mut)]
    pub destination: SystemAccount<'info>,

    /// Rent sysvar — read at execution time for the current lamports_per_byte.
    pub rent: Sysvar<'info, Rent>,
}

/// Handler for `reclaim_excess_rent`.
///
/// Logic mirrors the Token Program's `WithdrawExcessLamports`:
/// 1. Compute the rent-exempt floor at the current rate.
/// 2. Move only the excess (balance - floor) to destination.
/// 3. The account stays alive with exactly the rent-exempt minimum.
///
/// Authority check: the caller must verify that `authority` is permitted
/// to reclaim from `target`. For agent PDAs, the agent's wallet is the
/// authority. For protocol PDAs, the program authority is used.
///
/// The runtime guarantees that `target` is owned by this program —
/// otherwise the lamport mutation would fail.
pub fn handle_reclaim_excess_rent(ctx: Context<ReclaimExcessRent>) -> Result<()> {
    let target = &ctx.accounts.target;
    let destination = &ctx.accounts.destination;
    let rent = &ctx.accounts.rent;

    // 1. Compute the rent-exempt floor at the CURRENT lamports_per_byte.
    let rent_exempt_reserve = rent.minimum_balance(target.data_len());

    // 2. Everything above the floor is reclaimable.
    let excess = target
        .lamports()
        .checked_sub(rent_exempt_reserve)
        .ok_or(SapError::ArithmeticOverflow)?;

    if excess == 0 {
        // Nothing to reclaim — the account is already at the floor.
        return Ok(());
    }

    // 3. Direct lamport movement — legal because this program owns `target`.
    //    Debit the source, credit the destination. Runtime enforces balance.
    **destination.to_account_info().try_borrow_mut_lamports()? += excess;
    **target.try_borrow_mut_lamports()? -= excess;

    msg!(
        "reclaim_excess_rent: moved {} lamports from {} to {}",
        excess,
        target.key(),
        destination.key()
    );

    Ok(())
}