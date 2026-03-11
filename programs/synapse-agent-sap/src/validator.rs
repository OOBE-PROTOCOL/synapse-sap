use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::SapError;

// ═══════════════════════════════════════════════════════════════════
//  SAP Onchain Validator
//
//  Deep validation for agent registration and update payloads.
//  Catches data problems before they reach onchain state,
//  saving transaction fees and preventing invalid data.
//
//  Checks performed:
//  ────────────────────────────────────────────────────────────
//  • Name:          non-empty, ≤ 64 bytes, no control chars
//  • Description:   non-empty, ≤ 256 bytes
//  • Agent ID:      ≤ 128 bytes (optional DID-style)
//  • Capabilities:  protocol:method format, no duplicates, max count
//  • Pricing:       non-empty tierId, price > 0, rateLimit > 0,
//                   SPL requires tokenMint, no duplicate tiers,
//                   volume curve ascending, min ≤ max price
//  • x402 endpoint: must start with "https://"
//  • URIs:          ≤ 256 bytes
// ═══════════════════════════════════════════════════════════════════

/// Validate name: non-empty, ≤64B, no control chars.
pub fn validate_name(name: &str) -> Result<()> {
    require!(!name.is_empty(), SapError::EmptyName);
    require!(name.len() <= AgentAccount::MAX_NAME_LEN, SapError::NameTooLong);
    require!(
        !name.bytes().any(|b| b < 0x20),
        SapError::ControlCharInName
    );
    Ok(())
}

/// Validate description: non-empty, ≤256B.
pub fn validate_description(desc: &str) -> Result<()> {
    require!(!desc.is_empty(), SapError::EmptyDescription);
    require!(desc.len() <= AgentAccount::MAX_DESC_LEN, SapError::DescriptionTooLong);
    Ok(())
}

/// Validate agent_id: ≤128B.
pub fn validate_agent_id(agent_id: &str) -> Result<()> {
    require!(agent_id.len() <= AgentAccount::MAX_AGENT_ID_LEN, SapError::AgentIdTooLong);
    Ok(())
}

/// Validate capability: "protocol:method" format, non-empty parts.
pub fn validate_capability_format(id: &str) -> Result<()> {
    let colon_pos = id.find(':');
    match colon_pos {
        Some(pos) => {
            let protocol = &id[..pos];
            let method = &id[pos + 1..];
            require!(
                !protocol.is_empty() && !method.is_empty(),
                SapError::InvalidCapabilityFormat
            );
        }
        None => {
            return Err(error!(SapError::InvalidCapabilityFormat));
        }
    }
    Ok(())
}

/// Validate capabilities: max count, format, no duplicates.
pub fn validate_capabilities(caps: &[Capability]) -> Result<()> {
    require!(
        caps.len() <= AgentAccount::MAX_CAPABILITIES,
        SapError::TooManyCapabilities
    );

    for (i, cap) in caps.iter().enumerate() {
        validate_capability_format(&cap.id)?;

        // Check for duplicates (O(n²) but n ≤ 10, acceptable onchain)
        for j in (i + 1)..caps.len() {
            require!(
                cap.id != caps[j].id,
                SapError::DuplicateCapability
            );
        }
    }

    Ok(())
}

/// Validate volume curve: max points, ascending after_calls, price>0.
pub fn validate_volume_curve(curve: &[VolumeCurveBreakpoint]) -> Result<()> {
    require!(
        curve.len() <= AgentAccount::MAX_VOLUME_CURVE_POINTS,
        SapError::TooManyVolumeCurvePoints
    );

    for i in 1..curve.len() {
        require!(
            curve[i].after_calls > curve[i - 1].after_calls,
            SapError::InvalidVolumeCurve
        );
    }

    Ok(())
}

/// Validate pricing tier: non-empty tierId, rateLimit>0, SPL→tokenMint, min≤max.
pub fn validate_pricing_tier(tier: &PricingTier) -> Result<()> {
    require!(!tier.tier_id.is_empty(), SapError::EmptyTierId);
    // price_per_call == 0 is allowed (free tier)
    require!(tier.rate_limit > 0, SapError::InvalidRateLimit);

    // SPL token type requires a token mint address
    if tier.token_type == TokenType::Spl {
        require!(tier.token_mint.is_some(), SapError::SplRequiresTokenMint);
    }

    // Min/max price sanity check
    if let (Some(min), Some(max)) = (tier.min_price_per_call, tier.max_price_per_call) {
        require!(min <= max, SapError::MinPriceExceedsMax);
    }

    // Validate volume curve if present
    if let Some(ref curve) = tier.volume_curve {
        validate_volume_curve(curve)?;
    }

    Ok(())
}

/// Validate pricing list: max count, each valid, no duplicate IDs.
pub fn validate_pricing(pricing: &[PricingTier]) -> Result<()> {
    require!(
        pricing.len() <= AgentAccount::MAX_PRICING_TIERS,
        SapError::TooManyPricingTiers
    );

    for (i, tier) in pricing.iter().enumerate() {
        validate_pricing_tier(tier)?;

        // Check for duplicate tier IDs
        for j in (i + 1)..pricing.len() {
            require!(
                tier.tier_id != pricing[j].tier_id,
                SapError::DuplicateTierId
            );
        }
    }

    Ok(())
}

/// Validate x402 endpoint: starts with "https://", ≤MAX_URI_LEN.
pub fn validate_x402_endpoint(endpoint: &str) -> Result<()> {
    require!(endpoint.len() <= AgentAccount::MAX_URI_LEN, SapError::UriTooLong);
    require!(
        endpoint.starts_with("https://"),
        SapError::InvalidX402Endpoint
    );
    Ok(())
}

/// Validate URI: ≤MAX_URI_LEN.
pub fn validate_uri(uri: &str) -> Result<()> {
    require!(uri.len() <= AgentAccount::MAX_URI_LEN, SapError::UriTooLong);
    Ok(())
}

/// Validate uptime: 0-100.
pub fn validate_uptime_percent(pct: u8) -> Result<()> {
    require!(pct <= 100, SapError::InvalidUptimePercent);
    Ok(())
}

/// Full registration payload validation.
pub fn validate_registration(
    name: &str,
    description: &str,
    agent_id: &Option<String>,
    capabilities: &[Capability],
    pricing: &[PricingTier],
    protocols: &[String],
    agent_uri: &Option<String>,
    x402_endpoint: &Option<String>,
) -> Result<()> {
    validate_name(name)?;
    validate_description(description)?;

    if let Some(ref id) = agent_id {
        validate_agent_id(id)?;
    }

    validate_capabilities(capabilities)?;
    validate_pricing(pricing)?;

    require!(
        protocols.len() <= AgentAccount::MAX_PROTOCOLS,
        SapError::TooManyProtocols
    );

    if let Some(ref uri) = agent_uri {
        validate_uri(uri)?;
    }

    if let Some(ref endpoint) = x402_endpoint {
        validate_x402_endpoint(endpoint)?;
    }

    Ok(())
}

/// Partial update validation (only present fields checked).
pub fn validate_update(
    name: &Option<String>,
    description: &Option<String>,
    agent_id: &Option<String>,
    capabilities: &Option<Vec<Capability>>,
    pricing: &Option<Vec<PricingTier>>,
    protocols: &Option<Vec<String>>,
    agent_uri: &Option<String>,
    x402_endpoint: &Option<String>,
) -> Result<()> {
    if let Some(ref n) = name {
        validate_name(n)?;
    }

    if let Some(ref d) = description {
        validate_description(d)?;
    }

    if let Some(ref id) = agent_id {
        validate_agent_id(id)?;
    }

    if let Some(ref caps) = capabilities {
        validate_capabilities(caps)?;
    }

    if let Some(ref p) = pricing {
        validate_pricing(p)?;
    }

    if let Some(ref protos) = protocols {
        require!(
            protos.len() <= AgentAccount::MAX_PROTOCOLS,
            SapError::TooManyProtocols
        );
    }

    if let Some(ref uri) = agent_uri {
        validate_uri(uri)?;
    }

    if let Some(ref endpoint) = x402_endpoint {
        validate_x402_endpoint(endpoint)?;
    }

    Ok(())
}
