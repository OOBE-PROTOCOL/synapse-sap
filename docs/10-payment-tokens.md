# 10 — Payment Token Allowlist (v0.2.0)

> **Status:** mandatory since program v0.2.0.
> **Enforced in:** `create_escrow`, `create_escrow_v2`.
> **Error:** `PaymentTokenNotAllowed`.

## Rationale

Open token whitelists let a malicious agent (or client) seed the protocol
with a fake mint they fully control:

* They can mint unlimited supply at zero cost → bogus escrow deposits
  inflate metrics, indexers, and reputation rankings.
* Settlement payouts are denominated in worthless units → the escrow
  appears "settled" while no real economic value moved.
* Indexers, analytics, and downstream dApps cannot easily distinguish
  fake-token volume from real volume.

Restricting to **SOL + USDC** keeps the protocol's trust assumptions
aligned with its accounting.

## Allowed mints

| Mint | Network | Decimals |
|------|---------|----------|
| `null` (native) | mainnet + devnet | 9 (SOL) |
| `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | mainnet | 6 (USDC) |
| `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` | devnet | 6 (USDC) |

## Adding a new payment token

The allowlist is a **constant** in `programs/synapse-agent-sap/src/state.rs`
and `synapse-sap-sdk/src/constants/payments.ts`. Adding a token requires:

1. Governance / multisig review (token must be high-trust, sufficient
   liquidity, censorship characteristics understood).
2. Bump program minor version (additive, no PDA changes).
3. Mirror constant in SDK + bump SDK minor version.

## Client-side guard

`EscrowModule.create()` and `EscrowV2Module.create()` call
`isAcceptedPaymentToken()` before submitting the transaction. This
short-circuits the failed RPC round-trip and gives a clear error:

```ts
import { isAcceptedPaymentToken } from "@oobe-protocol-labs/synapse-sap-sdk";

if (!isAcceptedPaymentToken(tokenMint)) {
  throw new Error("Token not in SAP allowlist (use SOL or USDC).");
}
```
