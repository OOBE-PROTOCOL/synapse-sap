use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  AGENT ATTESTATION — Web of Trust
//
//  Third-party verifiable trust signals for agents.
//  Anyone can attest for any agent (one attestation per pair).
//
//  Unlike Feedback (user reviews, score-based, many-to-one),
//  Attestation is institutional trust:
//    - "Jupiter: API verified"
//    - "OtterSec: code audited"
//    - "Solana Foundation: official partner"
//    - "Chainlink: data feed certified"
//
//  Trust comes from WHO is attesting (their wallet identity),
//  not from the attestation itself.
//
//  Seeds: ["sap_attest", agent_pda, attester_wallet]
//  One attestation per (agent, attester) pair.
//  The metadata_hash points to detailed evidence offchain.
//
//  Lifecycle: create → (optional) revoke → close
// ═══════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────
//  create_attestation — Vouch for an agent
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CreateAttestationAccountConstraints<'info> {
    #[account(mut)]
    pub attester: Signer<'info>,

    /// Agent being attested — attester must NOT be owner
    #[account(
        constraint = attester.key() != agent.wallet @ SapError::SelfAttestationNotAllowed,
    )]
    pub agent: Account<'info, AgentAccount>,

    #[account(
        init,
        payer = attester,
        space = AgentAttestation::DISCRIMINATOR.len() + AgentAttestation::INIT_SPACE,
        seeds = [b"sap_attest", agent.key().as_ref(), attester.key().as_ref()],
        bump,
    )]
    pub attestation: Account<'info, AgentAttestation>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,

    pub system_program: Program<'info, System>,
}

pub fn create_attestation_handler(
    ctx: Context<CreateAttestationAccountConstraints>,
    attestation_type: String,
    metadata_hash: [u8; 32],
    expires_at: i64,
) -> Result<()> {
    require!(!attestation_type.is_empty(), SapError::EmptyAttestationType);
    require!(
        attestation_type.len() <= AgentAttestation::MAX_TYPE_LEN,
        SapError::AttestationTypeTooLong
    );

    let clock = Clock::get()?;

    // Validate expiry: 0 = never expires, > 0 must be in the future
    if expires_at > 0 {
        require!(
            expires_at > clock.unix_timestamp,
            SapError::AttestationExpired
        );
    }

    let att = &mut ctx.accounts.attestation;
    att.bump = ctx.bumps.attestation;
    att.agent = ctx.accounts.agent.key();
    att.attester = ctx.accounts.attester.key();
    att.attestation_type = attestation_type.clone();
    att.metadata_hash = metadata_hash;
    att.is_active = true;
    att.expires_at = expires_at;
    att.created_at = clock.unix_timestamp;
    att.updated_at = clock.unix_timestamp;

    ctx.accounts.global_registry.total_attestations = ctx.accounts.global_registry.total_attestations
        .checked_add(1).ok_or(error!(SapError::ArithmeticOverflow))?;

    emit!(AttestationCreatedEvent {
        agent: ctx.accounts.agent.key(),
        attester: ctx.accounts.attester.key(),
        attestation_type,
        expires_at,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  revoke_attestation — Revoke a previously issued attestation
//  Only the original attester can revoke.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct RevokeAttestationAccountConstraints<'info> {
    pub attester: Signer<'info>,

    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        seeds = [b"sap_attest", agent.key().as_ref(), attester.key().as_ref()],
        bump = attestation.bump,
        has_one = attester,
        has_one = agent,
        constraint = attestation.is_active @ SapError::AttestationAlreadyRevoked,
    )]
    pub attestation: Account<'info, AgentAttestation>,
}

pub fn revoke_attestation_handler(ctx: Context<RevokeAttestationAccountConstraints>) -> Result<()> {
    let clock = Clock::get()?;

    let att = &mut ctx.accounts.attestation;
    att.is_active = false;
    att.updated_at = clock.unix_timestamp;

    emit!(AttestationRevokedEvent {
        agent: ctx.accounts.agent.key(),
        attester: ctx.accounts.attester.key(),
        attestation_type: att.attestation_type.clone(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────
//  close_attestation — Close a revoked attestation PDA
//  Rent returned to the attester.  Must be revoked first.
// ─────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct CloseAttestationAccountConstraints<'info> {
    #[account(mut)]
    pub attester: Signer<'info>,

    pub agent: Account<'info, AgentAccount>,

    #[account(
        mut,
        close = attester,
        seeds = [b"sap_attest", agent.key().as_ref(), attester.key().as_ref()],
        bump = attestation.bump,
        has_one = attester,
        has_one = agent,
        constraint = !attestation.is_active @ SapError::AttestationNotRevoked,
    )]
    pub attestation: Account<'info, AgentAttestation>,

    #[account(
        mut,
        seeds = [b"sap_global"],
        bump = global_registry.bump,
    )]
    pub global_registry: Account<'info, GlobalRegistry>,
}

pub fn close_attestation_handler(ctx: Context<CloseAttestationAccountConstraints>) -> Result<()> {
    ctx.accounts.global_registry.total_attestations = ctx.accounts.global_registry.total_attestations.saturating_sub(1);
    Ok(())
}
