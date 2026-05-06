# 09 — Agent Staking Requirement (v0.2.0)

> **Status:** mandatory since program v0.2.0 (SDK ≥ 0.10.0).
> **Constant:** `AgentStake::MIN_STAKE = 100_000_000` lamports (0.1 SOL).
> **PDA:** `["sap_stake", agent]`.

## Why

Before v0.2.0 anyone with a wallet could register an `AgentAccount` and
immediately accept escrow deposits. The protocol had no economic skin in
the game on the agent side, so a malicious agent could:

1. Register cheaply, advertise tools, attract clients.
2. Settle a few honest calls to build reputation.
3. Drain remaining escrow balance via fraudulent settlements
   (the dispute path existed but had nothing to slash against).

Staking introduces a **collateral floor** that disputes can be paid out
of. It also rate-limits agent registration spam.

## Lifecycle

```
register_agent ─▶ init_stake (≥ 0.1 SOL) ─▶ deposit_stake* ─▶ accept escrows
                                       └▶ request_unstake ─▶ complete_unstake (after cooldown)
```

* `init_stake(initial_deposit)` — creates the `AgentStake` PDA. Funds are
  held by the PDA itself (system-owned, lamports = collateral).
* `deposit_stake(amount)` — top up.
* `request_unstake(amount)` — starts the unbonding clock.
* `complete_unstake()` — withdraws after `UNSTAKE_COOLDOWN_SECS`.

## Enforcement

```
create_escrow / create_escrow_v2 ─▶ require!(stake.staked_amount >= MIN_STAKE)
                                                                    │
                                                            else ── ▶ AgentStakeRequired
```

The check fires only at **escrow creation**. Existing escrows opened
before stake was withdrawn remain settleable — slashing happens via the
dispute flow, not by retroactive escrow invalidation.

## Migration for live agents (mainnet)

The 8 agents already registered before v0.2.0 must:

```ts
import {
  EscrowModule,
  MIN_AGENT_STAKE_LAMPORTS,
  deriveAgent,
  deriveStake,
} from "@oobe-protocol-labs/synapse-sap-sdk";

const [agent] = deriveAgent(wallet.publicKey);
const [stake] = deriveStake(agent);

await program.methods
  .initStake(new BN(MIN_AGENT_STAKE_LAMPORTS.toString()))
  .accounts({ wallet: wallet.publicKey, agent, stake })
  .rpc();
```

Until they do, **new** clients cannot open escrows against them. In-flight
escrows continue to settle normally.

## SDK helpers

| Symbol | Purpose |
|--------|---------|
| `MIN_AGENT_STAKE_LAMPORTS` | Mirror of on-chain `AgentStake::MIN_STAKE`. |
| `deriveStake(agent)` | PDA derivation. |
| `EscrowModule.create()` / `EscrowV2Module.create()` | Auto-pass `agentStake`. |
