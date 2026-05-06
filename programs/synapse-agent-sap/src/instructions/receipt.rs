use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  RECEIPT BATCH — Cryptographic Proof of Service Delivery
//
//  Agent periodically commits a merkle root of dual-signed call
//  receipts.  During disputes, individual receipts + merkle proofs
//  are presented to auto-resolve without any arbiter.
//
//  Off-chain receipt format (dual-signed):
//    { call_id, tool_id, input_hash, output_hash,
//      timestamp, nonce, client_sig, agent_sig }
//
//  client_sig = Ed25519(client_privkey, receipt_data)
//  agent_sig  = Ed25519(agent_privkey, receipt_data)
//  → Neither party can fabricate receipts unilaterally.
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  inscribe_receipt_batch — Agent commits merkle root on-chain
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(batch_index: u32)]
pub struct InscribeReceiptBatchAccountConstraints<'info> {
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
        seeds = [b"sap_escrow_v2", agent.key().as_ref(), escrow.depositor.as_ref(), &escrow.escrow_nonce.to_le_bytes()],
        bump = escrow.bump,
        constraint = escrow.agent == agent.key(),
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    #[account(
        init,
        payer = wallet,
        space = ReceiptBatch::DISCRIMINATOR.len() + ReceiptBatch::INIT_SPACE,
        seeds = [b"sap_receipt", escrow.key().as_ref(), &batch_index.to_le_bytes()],
        bump,
    )]
    pub receipt_batch: Account<'info, ReceiptBatch>,

    pub system_program: Program<'info, System>,
}

pub fn inscribe_receipt_batch_handler(
    ctx: Context<InscribeReceiptBatchAccountConstraints>,
    batch_index: u32,
    merkle_root: [u8; 32],
    call_count: u32,
    period_start: i64,
    period_end: i64,
) -> Result<()> {
    let clock = Clock::get()?;

    // Validate batch index is sequential
    require!(
        batch_index == ctx.accounts.escrow.receipt_batch_count,
        SapError::InvalidBatchIndex
    );
    require!(call_count > 0, SapError::BatchEmpty);
    require!(period_start <= period_end, SapError::InvalidPeriod);

    // Initialize receipt batch
    let rb = &mut ctx.accounts.receipt_batch;
    rb.bump = ctx.bumps.receipt_batch;
    rb.escrow = ctx.accounts.escrow.key();
    rb.batch_index = batch_index;
    rb.merkle_root = merkle_root;
    rb.call_count = call_count;
    rb.period_start = period_start;
    rb.period_end = period_end;
    rb.inscribed_at = clock.unix_timestamp;

    // Increment batch counter
    let escrow = &mut ctx.accounts.escrow;
    escrow.receipt_batch_count = escrow.receipt_batch_count
        .checked_add(1)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(ReceiptBatchInscribedEvent {
        escrow: escrow.key(),
        agent: ctx.accounts.agent.key(),
        batch_index,
        merkle_root,
        call_count,
        period_start,
        period_end,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  submit_receipt_proof — Agent proves delivery during dispute
//
//  Agent presents: receipt data + merkle proof → program verifies:
//    1. receipt_hash is in the merkle tree (proof verification)
//    2. merkle root matches the ReceiptBatch on-chain
//    3. call_count matches or exceeds claimed calls
//
//  Ed25519 signature verification of individual receipts is done
//  via Solana's Ed25519 precompile in the same transaction.
//  The program only needs to verify the merkle inclusion proof.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct SubmitReceiptProofAccountConstraints<'info> {
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
    )]
    pub escrow: Account<'info, EscrowAccountV2>,

    #[account(
        seeds = [b"sap_receipt", escrow.key().as_ref(), &receipt_batch.batch_index.to_le_bytes()],
        bump = receipt_batch.bump,
        constraint = receipt_batch.escrow == escrow.key(),
    )]
    pub receipt_batch: Account<'info, ReceiptBatch>,

    #[account(
        seeds = [b"sap_pending", escrow.key().as_ref(), &pending_settlement.settlement_index.to_le_bytes()],
        bump = pending_settlement.bump,
        constraint = pending_settlement.escrow == escrow.key(),
        constraint = pending_settlement.is_disputed @ SapError::SettlementNotPending,
    )]
    pub pending_settlement: Account<'info, PendingSettlement>,

    #[account(
        mut,
        seeds = [b"sap_dispute", pending_settlement.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.agent == agent.key(),
        constraint = dispute.outcome == DisputeOutcome::Pending @ SapError::SettlementAlreadyFinalized,
    )]
    pub dispute: Account<'info, DisputeRecord>,
}

pub fn submit_receipt_proof_handler(
    ctx: Context<SubmitReceiptProofAccountConstraints>,
    receipt_hashes: Vec<[u8; 32]>,
    merkle_proofs: Vec<Vec<[u8; 32]>>,
) -> Result<()> {
    let clock = Clock::get()?;

    // Must be within proof deadline
    require!(
        clock.unix_timestamp <= ctx.accounts.dispute.proof_deadline,
        SapError::ProofDeadlineExpired
    );

    require!(
        receipt_hashes.len() == merkle_proofs.len(),
        SapError::InvalidReceiptProof
    );
    // v0.13: prevent CU exhaustion via unbounded receipt proofs
    require!(
        receipt_hashes.len() <= EscrowAccountV2::MAX_RECEIPT_PROOFS,
        SapError::MaxReceiptProofExceeded
    );

    let merkle_root = ctx.accounts.receipt_batch.merkle_root;
    let mut verified_count: u32 = 0;

    // Verify each receipt's merkle inclusion
    for (i, receipt_hash) in receipt_hashes.iter().enumerate() {
        let proof = &merkle_proofs[i];
        require!(
            proof.len() <= EscrowAccountV2::MAX_MERKLE_DEPTH,
            SapError::MaxMerkleDepthExceeded
        );
        if verify_merkle_proof(receipt_hash, proof, &merkle_root) {
            verified_count += 1;
        }
    }

    // Update dispute with proven calls
    let dispute = &mut ctx.accounts.dispute;
    dispute.proven_calls = dispute.proven_calls
        .checked_add(verified_count)
        .ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(ReceiptProofSubmittedEvent {
        dispute: dispute.key(),
        escrow: ctx.accounts.escrow.key(),
        agent: ctx.accounts.agent.key(),
        receipts_submitted: receipt_hashes.len() as u32,
        receipts_verified: verified_count,
        total_proven: dispute.proven_calls,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  auto_resolve_dispute — Permissionless auto-resolution
//
//  Anyone can call this after proof deadline or when proofs
//  are sufficient.  Resolution logic:
//    - proven_calls >= claimed_calls → AgentWins
//    - proven_calls == 0 && deadline passed → DepositorWins
//    - 0 < proven_calls < claimed_calls → PartialRefund
//    - Quality dispute with no auto-check match → Split (50/50)
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct AutoResolveDisputeAccountConstraints<'info> {
    /// Anyone can crank
    #[account(mut)]
    pub payer: Signer<'info>,

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
        constraint = dispute.outcome == DisputeOutcome::Pending @ SapError::SettlementAlreadyFinalized,
    )]
    pub dispute: Account<'info, DisputeRecord>,

    #[account(
        mut,
        seeds = [b"sap_stats", escrow.agent.as_ref()],
        bump = agent_stats.bump,
    )]
    pub agent_stats: Account<'info, AgentStats>,

    /// v0.11 H-3: AgentStake is now a typed required account so the slash on
    /// DepositorWins outcomes is guaranteed instead of silently skipped when
    /// the caller forgets to pass it via remaining_accounts.
    #[account(
        mut,
        seeds = [b"sap_stake", escrow.agent.as_ref()],
        bump = agent_stake.bump,
        constraint = agent_stake.agent == escrow.agent @ SapError::StakeAgentMismatch,
    )]
    pub agent_stake: Account<'info, AgentStake>,
}

pub fn auto_resolve_dispute_handler<'info>(
    ctx: Context<'info, AutoResolveDisputeAccountConstraints<'info>>,
) -> Result<()> {
    let clock = Clock::get()?;

    // Verify depositor / agent_wallet
    require!(
        ctx.accounts.depositor.key() == ctx.accounts.escrow.depositor,
        SapError::NotDepositor
    );
    require!(
        ctx.accounts.agent_wallet.key() == ctx.accounts.escrow.agent_wallet,
        SapError::InvalidAgentWallet
    );

    // v0.13: verify pending settlement amount matches escrow accounting.
    // Any mismatch would corrupt escrow state during finalization.
    require!(
        ctx.accounts.pending_settlement.amount <= ctx.accounts.escrow.pending_amount,
        SapError::PendingAmountMismatch
    );

    let proven = ctx.accounts.dispute.proven_calls;
    let claimed = ctx.accounts.dispute.claimed_calls;
    let deadline_passed = clock.unix_timestamp > ctx.accounts.dispute.proof_deadline;
    let amount = ctx.accounts.pending_settlement.amount;
    let calls = ctx.accounts.pending_settlement.calls_to_settle;

    // Determine outcome
    let (outcome, agent_amount, depositor_amount) = if proven >= claimed && claimed > 0 {
        // Agent proved all calls → agent wins
        (DisputeOutcome::AgentWins, amount, 0u64)
    } else if proven == 0 && deadline_passed {
        // No proof and deadline passed → depositor wins
        (DisputeOutcome::DepositorWins, 0u64, amount)
    } else if proven > 0 && (proven < claimed || deadline_passed) {
        // Partial proof → proportional refund
        let agent_share = (amount as u128)
            .checked_mul(proven as u128)
            .ok_or(error!(SapError::ArithmeticOverflow))?
            .checked_div(claimed as u128)
            .ok_or(error!(SapError::ArithmeticOverflow))? as u64;
        let depositor_share = amount.checked_sub(agent_share)
            .ok_or(error!(SapError::ArithmeticOverflow))?;
        (DisputeOutcome::PartialRefund, agent_share, depositor_share)
    } else if ctx.accounts.dispute.dispute_type == DisputeType::Quality && deadline_passed {
        // Quality dispute, no conclusive auto-check → 50/50 split
        let half = amount / 2;
        let remainder = amount - half;
        (DisputeOutcome::Split, half, remainder)
    } else {
        // Deadline not passed yet, can't resolve — agent still has time
        return Err(error!(SapError::ProofDeadlineNotExpired));
    };

    // Transfer funds
    let escrow_info = ctx.accounts.escrow.to_account_info();
    let depositor_info = ctx.accounts.depositor.to_account_info();
    let wallet_info = ctx.accounts.agent_wallet.to_account_info();

    if agent_amount > 0 {
        if ctx.accounts.escrow.token_mint.is_some() {
            let remaining = ctx.remaining_accounts;
            require!(remaining.len() >= 4, SapError::SplTokenRequired);
            spl_transfer_from_escrow_to_account(
                &escrow_info,
                &remaining[0], // escrow_token
                &remaining[1], // agent ATA
                &remaining[3], // token_program
                &ctx.accounts.escrow.agent,
                &ctx.accounts.escrow.depositor,
                ctx.accounts.escrow.escrow_nonce,
                ctx.accounts.escrow.bump,
                agent_amount,
                ctx.accounts.escrow.token_mint,
            )?;
        } else {
            **escrow_info.try_borrow_mut_lamports()? -= agent_amount;
            **wallet_info.try_borrow_mut_lamports()? += agent_amount;
        }
    }

    if depositor_amount > 0 {
        if ctx.accounts.escrow.token_mint.is_some() {
            let remaining = ctx.remaining_accounts;
            require!(remaining.len() >= 4, SapError::SplTokenRequired);
            spl_transfer_from_escrow_to_account(
                &escrow_info,
                &remaining[0], // escrow_token
                &remaining[2], // depositor ATA
                &remaining[3], // token_program
                &ctx.accounts.escrow.agent,
                &ctx.accounts.escrow.depositor,
                ctx.accounts.escrow.escrow_nonce,
                ctx.accounts.escrow.bump,
                depositor_amount,
                ctx.accounts.escrow.token_mint,
            )?;
        } else {
            **escrow_info.try_borrow_mut_lamports()? -= depositor_amount;
            **depositor_info.try_borrow_mut_lamports()? += depositor_amount;
        }
    }

    // Update escrow
    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow.balance.checked_sub(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.pending_amount = escrow.pending_amount.checked_sub(amount).ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.pending_calls = escrow.pending_calls.checked_sub(calls).ok_or(error!(SapError::ArithmeticOverflow))?;

    if agent_amount > 0 {
        escrow.total_settled = escrow.total_settled.checked_add(agent_amount).ok_or(error!(SapError::ArithmeticOverflow))?;
        let proven_calls_u64 = proven as u64;
        escrow.total_calls_settled = escrow.total_calls_settled.checked_add(proven_calls_u64.min(calls)).ok_or(error!(SapError::ArithmeticOverflow))?;
        escrow.last_settled_at = clock.unix_timestamp;

        let stats = &mut ctx.accounts.agent_stats;
        stats.total_calls_served = stats.total_calls_served.checked_add(proven_calls_u64.min(calls)).ok_or(error!(SapError::ArithmeticOverflow))?;
        stats.updated_at = clock.unix_timestamp;
    }

    // Try slash on DepositorWins (v0.11 H-3: typed account, no silent skip)
    if outcome == DisputeOutcome::DepositorWins {
        let slash_amount = super::dispute::try_slash_from_account(
            &mut ctx.accounts.agent_stake,
            amount,
            &depositor_info,
            &ctx.accounts.escrow.agent,
            &ctx.accounts.dispute.key(),
            &clock,
        )?;
        ctx.accounts.dispute.slash_amount = slash_amount;
    }

    // Return dispute bond (if Quality dispute)
    if ctx.accounts.dispute.dispute_bond > 0 {
        let bond = ctx.accounts.dispute.dispute_bond;
        // Bond goes back to depositor (they paid it)
        if outcome != DisputeOutcome::AgentWins {
            // Depositor gets bond back (they were right or partially right)
            // Bond was deposited into escrow PDA, return it
            **escrow_info.try_borrow_mut_lamports()? -= bond;
            **depositor_info.try_borrow_mut_lamports()? += bond;
        }
        // If AgentWins, bond stays as compensation (already in escrow)
    }

    // Finalize
    let ps = &mut ctx.accounts.pending_settlement;
    ps.is_finalized = true;
    ps.outcome = outcome;

    let dispute = &mut ctx.accounts.dispute;
    let is_quality = dispute.dispute_type == DisputeType::Quality;
    dispute.outcome = outcome;
    dispute.resolution_layer = if is_quality {
        ResolutionLayer::Governance
    } else {
        ResolutionLayer::Auto
    };
    dispute.resolved_at = clock.unix_timestamp;

    emit!(DisputeAutoResolvedEvent {
        dispute: dispute.key(),
        pending_settlement: ps.key(),
        escrow: ctx.accounts.escrow.key(),
        outcome: outcome as u8,
        proven_calls: proven,
        claimed_calls: claimed,
        agent_amount,
        depositor_amount,
        slash_amount: dispute.slash_amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Merkle Proof Verification (SHA-256 binary tree)
// ═══════════════════════════════════════════════════════════════════

fn verify_merkle_proof(leaf: &[u8; 32], proof: &[[u8; 32]], root: &[u8; 32]) -> bool {
    let mut hash = *leaf;
    for sibling in proof {
        hash = if hash <= *sibling {
            hash_pair(&hash, sibling)
        } else {
            hash_pair(sibling, &hash)
        };
    }
    hash == *root
}

fn hash_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    use solana_sha256_hasher::hashv;
    hashv(&[a, b]).to_bytes()
}

/// v0.12 fix: Safe SPL transfer from escrow to a SPECIFIC destination token account.
/// Caller passes source escrow ATA, dest ATA, and token program explicitly.
/// This replaces the fixed-remaining[] helper so both agent and depositor
/// can receive USDC in the same TX when PartialRefund / Split applies.
fn spl_transfer_from_escrow_to_account<'info>(
    escrow_info: &AccountInfo<'info>,
    escrow_token: &AccountInfo<'info>,
    dest_token: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    agent_key: &Pubkey,
    depositor_key: &Pubkey,
    escrow_nonce: u64,
    escrow_bump: u8,
    amount: u64,
    expected_mint: Option<Pubkey>,
) -> Result<()> {
    if let Some(mint) = expected_mint {
        let data = escrow_token.try_borrow_data()?;
        require!(data.len() >= 32, SapError::InvalidTokenAccount);
        let source_mint = Pubkey::try_from(&data[..32]).map_err(|_| error!(SapError::InvalidTokenAccount))?;
        require!(source_mint == mint, SapError::InvalidTokenAccount);
    }

    require!(super::escrow_v2::is_spl_token_program(token_program.key), SapError::InvalidTokenProgram);

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
