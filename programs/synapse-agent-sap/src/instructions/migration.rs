use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  MIGRATION — Clean Account Upgrades from V1 → V2
//
//  Mainnet-safe:  Existing V1 accounts remain untouched.
//  Migration creates NEW V2 PDAs while marking the old account
//  as migrated (setting a flag byte).
//
//  migrate_escrow_v1_to_v2:
//    - Reads V1 EscrowAccount data
//    - Creates EscrowAccountV2 with nonce=0, security=SelfReport
//    - Transfers balance from V1 → V2
//    - Marks V1 as migrated (closes it, refunding depositor)
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  migrate_escrow_v1_to_v2
//
//  Depositor (owner of V1 escrow) initiates migration.
//  V1 balance is swept into V2 escrow.  V1 account is closed.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct MigrateEscrowV1ToV2AccountConstraints<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        constraint = agent.is_active @ SapError::AgentInactive,
    )]
    pub agent: Account<'info, AgentAccount>,

    /// V1 escrow being migrated — will be closed
    #[account(
        mut,
        close = depositor,
        seeds = [b"sap_escrow", agent.key().as_ref(), depositor.key().as_ref()],
        bump = escrow_v1.bump,
        has_one = depositor,
        constraint = escrow_v1.agent == agent.key(),
    )]
    pub escrow_v1: Account<'info, EscrowAccount>,

    /// V2 escrow — init with nonce=0 (first escrow for this pair)
    #[account(
        init,
        payer = depositor,
        space = EscrowAccountV2::DISCRIMINATOR.len() + EscrowAccountV2::INIT_SPACE,
        seeds = [b"sap_escrow_v2", agent.key().as_ref(), depositor.key().as_ref(), &0u64.to_le_bytes()],
        bump,
    )]
    pub escrow_v2: Account<'info, EscrowAccountV2>,

    pub system_program: Program<'info, System>,
}

pub fn migrate_escrow_v1_to_v2_handler(
    ctx: Context<MigrateEscrowV1ToV2AccountConstraints>,
) -> Result<()> {
    let clock = Clock::get()?;

    let v1 = &ctx.accounts.escrow_v1;

    // Copy V1 state into V2
    let v2 = &mut ctx.accounts.escrow_v2;
    v2.bump = ctx.bumps.escrow_v2;
    v2.version = EscrowAccountV2::VERSION;
    v2.agent = v1.agent;
    v2.depositor = v1.depositor;
    v2.agent_wallet = v1.agent_wallet;
    v2.escrow_nonce = 0; // First V2 escrow for this pair
    v2.balance = v1.balance;
    v2.total_deposited = v1.total_deposited;
    v2.total_settled = v1.total_settled;
    v2.total_calls_settled = v1.total_calls_settled;
    v2.price_per_call = v1.price_per_call;
    v2.max_calls = v1.max_calls;
    v2.created_at = v1.created_at;
    v2.last_settled_at = v1.last_settled_at;
    v2.expires_at = v1.expires_at;
    v2.token_mint = v1.token_mint;
    v2.token_decimals = v1.token_decimals;

    // Migrate volume curve
    v2.volume_curve = v1.volume_curve.clone();

    // New V2 fields — default to SelfReport (backward-compat)
    v2.settlement_security = SettlementSecurity::SelfReport;
    v2.dispute_window_slots = 0;
    v2.settlement_index = 0;
    v2.co_signer = None;
    v2.arbiter = None;
    v2.pending_amount = 0;
    v2.pending_calls = 0;

    // Transfer any SOL balance from V1 → V2.
    // Done BEFORE Anchor close reclaims remaining lamports to depositor.
    // C2 fix: Verify V1 has enough lamports for both the balance transfer
    // AND rent-exempt minimum (Anchor close needs the PDA to remain valid).
    if v1.balance > 0 && v1.token_mint.is_none() {
        let v1_info = ctx.accounts.escrow_v1.to_account_info();
        let v2_info = ctx.accounts.escrow_v2.to_account_info();
        let v1_lamports = v1_info.lamports();
        let rent = Rent::get()?;
        let v1_rent = rent.minimum_balance(v1_info.data_len());

        // Only transfer what's actually available above rent
        let transferable = v1_lamports.saturating_sub(v1_rent);
        let actual_transfer = v1.balance.min(transferable);

        if actual_transfer > 0 {
            **v1_info.try_borrow_mut_lamports()? -= actual_transfer;
            **v2_info.try_borrow_mut_lamports()? += actual_transfer;
        }

        // Update V2 balance to actual transferred amount
        ctx.accounts.escrow_v2.balance = actual_transfer;
    }

    emit!(AccountMigratedEvent {
        account: ctx.accounts.escrow_v1.key(),
        account_type: String::from("EscrowV1→V2"),
        from_version: 1,
        to_version: 2,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
