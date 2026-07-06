use crate::errors::SapError;
use crate::events::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use solana_instructions_sysvar as ix_sysvar;
use solana_sdk_ids::ed25519_program;

const RECEIPT_SIGNATURE_DOMAIN: &[u8] = b"SAP_RECEIPT_V1";
const ED25519_SIGNATURE_OFFSETS_START: usize = 2;
const ED25519_SIGNATURE_OFFSETS_SIZE: usize = 14;
const ED25519_SIGNATURE_SIZE: usize = 64;
const ED25519_PUBKEY_SIZE: usize = 32;
const ED25519_DATA_IN_THIS_INSTRUCTION: u16 = u16::MAX;

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
    escrow.receipt_batch_count = escrow
        .receipt_batch_count
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
    require!(!receipt_hashes.is_empty(), SapError::InvalidReceiptProof);
    // v0.13: prevent CU exhaustion via unbounded receipt proofs
    require!(
        receipt_hashes.len() <= EscrowAccountV2::MAX_RECEIPT_PROOFS,
        SapError::MaxReceiptProofExceeded
    );
    require!(
        ctx.accounts.dispute.proven_calls == 0,
        SapError::ReceiptProofAlreadySubmitted
    );

    let merkle_root = ctx.accounts.receipt_batch.merkle_root;
    let mut verified_count: u32 = 0;
    let instructions_sysvar = find_instructions_sysvar(ctx.remaining_accounts)?;
    let current_ix = ix_sysvar::load_current_index_checked(instructions_sysvar)?;

    // Verify each receipt's merkle inclusion
    for (i, receipt_hash) in receipt_hashes.iter().enumerate() {
        for prior_hash in receipt_hashes.iter().take(i) {
            require!(prior_hash != receipt_hash, SapError::DuplicateReceiptProof);
        }

        let proof = &merkle_proofs[i];
        require!(
            proof.len() <= EscrowAccountV2::MAX_MERKLE_DEPTH,
            SapError::MaxMerkleDepthExceeded
        );
        require!(
            verify_merkle_proof(receipt_hash, proof, &merkle_root),
            SapError::InvalidReceiptProof
        );

        let message = build_receipt_signature_message(
            &ctx.accounts.escrow.key(),
            &ctx.accounts.pending_settlement.key(),
            &ctx.accounts.dispute.key(),
            receipt_hash,
        );
        require!(
            has_verified_ed25519_signature_before(
                instructions_sysvar,
                current_ix,
                &ctx.accounts.escrow.depositor,
                &message,
            )?,
            SapError::MissingReceiptSignature
        );
        require!(
            has_verified_ed25519_signature_before(
                instructions_sysvar,
                current_ix,
                &ctx.accounts.escrow.agent_wallet,
                &message,
            )?,
            SapError::MissingReceiptSignature
        );

        verified_count = verified_count
            .checked_add(1)
            .ok_or(error!(SapError::ArithmeticOverflow))?;
    }

    // Update dispute with proven calls
    let dispute = &mut ctx.accounts.dispute;
    dispute.proven_calls = verified_count.min(dispute.claimed_calls);

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

    let (outcome, agent_amount, depositor_amount) = determine_dispute_resolution(
        ctx.accounts.dispute.dispute_type,
        proven,
        claimed,
        deadline_passed,
        amount,
    )?;

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
                Some(ctx.accounts.escrow.agent_wallet),
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
                Some(ctx.accounts.escrow.depositor),
            )?;
        } else {
            **escrow_info.try_borrow_mut_lamports()? -= depositor_amount;
            **depositor_info.try_borrow_mut_lamports()? += depositor_amount;
        }
    }

    // Update escrow
    let escrow = &mut ctx.accounts.escrow;
    escrow.balance = escrow
        .balance
        .checked_sub(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.pending_amount = escrow
        .pending_amount
        .checked_sub(amount)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.pending_calls = escrow
        .pending_calls
        .checked_sub(calls)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    escrow.pending_settlement_count = escrow.pending_settlement_count.saturating_sub(1);

    if agent_amount > 0 {
        escrow.total_settled = escrow
            .total_settled
            .checked_add(agent_amount)
            .ok_or(error!(SapError::ArithmeticOverflow))?;
        let proven_calls_u64 = proven as u64;
        escrow.total_calls_settled = escrow
            .total_calls_settled
            .checked_add(proven_calls_u64.min(calls))
            .ok_or(error!(SapError::ArithmeticOverflow))?;
        escrow.last_settled_at = clock.unix_timestamp;

        let stats = &mut ctx.accounts.agent_stats;
        stats.total_calls_served = stats
            .total_calls_served
            .checked_add(proven_calls_u64.min(calls))
            .ok_or(error!(SapError::ArithmeticOverflow))?;
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
        **escrow_info.try_borrow_mut_lamports()? -= bond;
        if outcome == DisputeOutcome::AgentWins {
            **wallet_info.try_borrow_mut_lamports()? += bond;
        } else {
            **depositor_info.try_borrow_mut_lamports()? += bond;
        }
        ctx.accounts.escrow.dispute_bond_total =
            ctx.accounts.escrow.dispute_bond_total.saturating_sub(bond);
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

fn determine_dispute_resolution(
    dispute_type: DisputeType,
    proven: u32,
    claimed: u32,
    deadline_passed: bool,
    amount: u64,
) -> Result<(DisputeOutcome, u64, u64)> {
    if dispute_type == DisputeType::Quality {
        if proven >= claimed && claimed > 0 {
            return Ok((DisputeOutcome::AgentWins, amount, 0));
        }
        if deadline_passed {
            let half = amount / 2;
            return Ok((DisputeOutcome::Split, half, amount - half));
        }
        return Err(error!(SapError::ProofDeadlineNotExpired));
    }

    if proven >= claimed && claimed > 0 {
        return Ok((DisputeOutcome::AgentWins, amount, 0));
    }
    if proven == 0 && deadline_passed {
        return Ok((DisputeOutcome::DepositorWins, 0, amount));
    }
    if proven > 0 && (proven < claimed || deadline_passed) {
        let agent_share = (amount as u128)
            .checked_mul(proven as u128)
            .ok_or(error!(SapError::ArithmeticOverflow))?
            .checked_div(claimed as u128)
            .ok_or(error!(SapError::ArithmeticOverflow))? as u64;
        let depositor_share = amount
            .checked_sub(agent_share)
            .ok_or(error!(SapError::ArithmeticOverflow))?;
        return Ok((DisputeOutcome::PartialRefund, agent_share, depositor_share));
    }

    Err(error!(SapError::ProofDeadlineNotExpired))
}

fn build_receipt_signature_message(
    escrow: &Pubkey,
    pending_settlement: &Pubkey,
    dispute: &Pubkey,
    receipt_hash: &[u8; 32],
) -> Vec<u8> {
    let mut message = Vec::with_capacity(RECEIPT_SIGNATURE_DOMAIN.len() + 32 + 32 + 32 + 32 + 32);
    message.extend_from_slice(RECEIPT_SIGNATURE_DOMAIN);
    message.extend_from_slice(crate::ID.as_ref());
    message.extend_from_slice(escrow.as_ref());
    message.extend_from_slice(pending_settlement.as_ref());
    message.extend_from_slice(dispute.as_ref());
    message.extend_from_slice(receipt_hash);
    message
}

fn has_verified_ed25519_signature_before(
    instructions_sysvar: &AccountInfo,
    current_ix: u16,
    signer: &Pubkey,
    message: &[u8],
) -> Result<bool> {
    for ix_index in 0..current_ix {
        let ix = ix_sysvar::load_instruction_at_checked(ix_index as usize, instructions_sysvar)?;
        if ed25519_instruction_contains(&ix, signer.as_ref(), message)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn find_instructions_sysvar<'info>(
    remaining_accounts: &'info [AccountInfo<'info>],
) -> Result<&'info AccountInfo<'info>> {
    remaining_accounts
        .iter()
        .find(|account| account.key.as_ref() == ix_sysvar::ID.as_ref())
        .ok_or(error!(SapError::MissingReceiptSignature))
}

fn ed25519_instruction_contains(ix: &Instruction, signer: &[u8], message: &[u8]) -> Result<bool> {
    if ix.program_id != ed25519_program::ID {
        return Ok(false);
    }
    let data = ix.data.as_slice();
    require!(
        data.len() >= ED25519_SIGNATURE_OFFSETS_START,
        SapError::InvalidReceiptProof
    );

    let sig_count = data[0] as usize;
    for sig_index in 0..sig_count {
        let offset = ED25519_SIGNATURE_OFFSETS_START
            .checked_add(
                sig_index
                    .checked_mul(ED25519_SIGNATURE_OFFSETS_SIZE)
                    .ok_or(error!(SapError::ArithmeticOverflow))?,
            )
            .ok_or(error!(SapError::ArithmeticOverflow))?;
        require!(
            data.len() >= offset + ED25519_SIGNATURE_OFFSETS_SIZE,
            SapError::InvalidReceiptProof
        );

        let signature_offset = read_u16(data, offset)? as usize;
        let signature_instruction_index = read_u16(data, offset + 2)?;
        let public_key_offset = read_u16(data, offset + 4)? as usize;
        let public_key_instruction_index = read_u16(data, offset + 6)?;
        let message_offset = read_u16(data, offset + 8)? as usize;
        let message_size = read_u16(data, offset + 10)? as usize;
        let message_instruction_index = read_u16(data, offset + 12)?;

        if signature_instruction_index != ED25519_DATA_IN_THIS_INSTRUCTION
            || public_key_instruction_index != ED25519_DATA_IN_THIS_INSTRUCTION
            || message_instruction_index != ED25519_DATA_IN_THIS_INSTRUCTION
        {
            continue;
        }

        require!(
            signature_offset
                .checked_add(ED25519_SIGNATURE_SIZE)
                .map(|end| end <= data.len())
                .unwrap_or(false),
            SapError::InvalidReceiptProof
        );
        require!(
            public_key_offset
                .checked_add(ED25519_PUBKEY_SIZE)
                .map(|end| end <= data.len())
                .unwrap_or(false),
            SapError::InvalidReceiptProof
        );
        require!(
            message_offset
                .checked_add(message_size)
                .map(|end| end <= data.len())
                .unwrap_or(false),
            SapError::InvalidReceiptProof
        );

        let public_key = &data[public_key_offset..public_key_offset + ED25519_PUBKEY_SIZE];
        let signed_message = &data[message_offset..message_offset + message_size];
        if public_key == signer && signed_message == message {
            return Ok(true);
        }
    }

    Ok(false)
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(error!(SapError::ArithmeticOverflow))?;
    require!(end <= data.len(), SapError::InvalidReceiptProof);
    Ok(u16::from_le_bytes([data[offset], data[offset + 1]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_ed25519_instruction(pubkey: Pubkey, message: &[u8]) -> Instruction {
        let signature_offset = 16u16;
        let public_key_offset = signature_offset + ED25519_SIGNATURE_SIZE as u16;
        let message_offset = public_key_offset + ED25519_PUBKEY_SIZE as u16;

        let mut data = Vec::new();
        data.push(1);
        data.push(0);
        data.extend_from_slice(&signature_offset.to_le_bytes());
        data.extend_from_slice(&ED25519_DATA_IN_THIS_INSTRUCTION.to_le_bytes());
        data.extend_from_slice(&public_key_offset.to_le_bytes());
        data.extend_from_slice(&ED25519_DATA_IN_THIS_INSTRUCTION.to_le_bytes());
        data.extend_from_slice(&message_offset.to_le_bytes());
        data.extend_from_slice(&(message.len() as u16).to_le_bytes());
        data.extend_from_slice(&ED25519_DATA_IN_THIS_INSTRUCTION.to_le_bytes());
        data.extend_from_slice(&[7u8; ED25519_SIGNATURE_SIZE]);
        data.extend_from_slice(pubkey.as_ref());
        data.extend_from_slice(message);

        Instruction {
            program_id: ed25519_program::ID,
            accounts: vec![],
            data,
        }
    }

    #[test]
    fn receipt_signature_message_is_domain_separated() {
        let escrow = Pubkey::new_unique();
        let pending = Pubkey::new_unique();
        let dispute = Pubkey::new_unique();
        let receipt_hash = [9u8; 32];

        let message = build_receipt_signature_message(&escrow, &pending, &dispute, &receipt_hash);

        assert!(message.starts_with(RECEIPT_SIGNATURE_DOMAIN));
        assert!(message
            .windows(32)
            .any(|window| window == crate::ID.as_ref()));
        assert!(message.windows(32).any(|window| window == escrow.as_ref()));
        assert!(message.windows(32).any(|window| window == pending.as_ref()));
        assert!(message.windows(32).any(|window| window == dispute.as_ref()));
        assert!(message.ends_with(&receipt_hash));
    }

    #[test]
    fn ed25519_parser_matches_expected_signer_and_message() {
        let signer = Pubkey::new_unique();
        let message = b"sap receipt message";
        let ix = fake_ed25519_instruction(signer, message);

        assert!(ed25519_instruction_contains(&ix, signer.as_ref(), message).unwrap());
        assert!(
            !ed25519_instruction_contains(&ix, Pubkey::new_unique().as_ref(), message).unwrap()
        );
        assert!(!ed25519_instruction_contains(&ix, signer.as_ref(), b"other").unwrap());
    }

    #[test]
    fn quality_dispute_splits_after_deadline_before_generic_refund_paths() {
        let (outcome, agent_amount, depositor_amount) =
            determine_dispute_resolution(DisputeType::Quality, 3, 10, true, 101).unwrap();

        assert!(outcome == DisputeOutcome::Split);
        assert_eq!(agent_amount, 50);
        assert_eq!(depositor_amount, 51);
    }
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
    expected_dest_owner: Option<Pubkey>,
) -> Result<()> {
    require!(
        super::escrow_v2::is_spl_token_program(token_program.key),
        SapError::InvalidTokenProgram
    );
    require!(
        escrow_token.key() != dest_token.key(),
        SapError::InvalidTokenAccount
    );
    super::escrow_v2::validate_token_account(
        escrow_token,
        expected_mint,
        Some(*escrow_info.key),
        token_program,
    )?;
    super::escrow_v2::validate_token_account(
        dest_token,
        expected_mint,
        expected_dest_owner,
        token_program,
    )?;

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
