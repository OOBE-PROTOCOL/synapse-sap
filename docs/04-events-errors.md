# Events & Error Codes

> **Synapse Agent Protocol (SAP) v2** ... 45 Events, 91 Errors  
> Program ID: `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`  
> Anchor 0.32.1 · Solana SVM

All SAP state mutations emit structured Anchor events to the transaction log. These events are
permanent, zero-rent, and form the backbone of the protocol's auditability and data indexing layer.
This document catalogues every event and error the program can produce.

---

## Table of Contents

**Events**
1. [Agent Events (7)](#agent-events)
2. [Feedback Events (3)](#feedback-events)
3. [Vault Events (11)](#vault-events)
4. [Tool Events (8)](#tool-events)
5. [Escrow Events (5)](#escrow-events)
6. [Attestation Events (2)](#attestation-events)
7. [Memory Events (9)](#memory-events)

**Errors**
8. [Error Codes (91)](#error-codes)

---

## Events

Anchor events are emitted via `emit!()` and serialized into the transaction's log data.
Parse them with `program.addEventListener("EventName", callback)` in the TypeScript client,
or decode from raw transaction logs using `program.coder.events.decode()`.

---

### Agent Events

7 events covering the full agent lifecycle.

| Event | Key Fields | Emitted By |
|-------|-----------|------------|
| `RegisteredEvent` | `agent`, `wallet`, `name`, `capabilities[]`, `timestamp` | `register_agent` |
| `UpdatedEvent` | `agent`, `wallet`, `updated_fields[]`, `timestamp` | `update_agent` |
| `DeactivatedEvent` | `agent`, `wallet`, `timestamp` | `deactivate_agent` |
| `ReactivatedEvent` | `agent`, `wallet`, `timestamp` | `reactivate_agent` |
| `ClosedEvent` | `agent`, `wallet`, `timestamp` | `close_agent` |
| `CallsReportedEvent` | `agent`, `wallet`, `calls_reported`, `total_calls_served`, `timestamp` | `report_calls` |
| `ReputationUpdatedEvent` | `agent`, `wallet`, `avg_latency_ms`, `uptime_percent`, `timestamp` | `update_reputation` |

**`UpdatedEvent.updated_fields`**: Strings listing which fields were changed (e.g., `["name", "pricing", "x402_endpoint"]`). Useful for incremental indexers that need to know what changed without diffing the full account.

---

### Feedback Events

3 events for the trustless reputation system.

| Event | Key Fields | Emitted By |
|-------|-----------|------------|
| `FeedbackEvent` | `agent`, `reviewer`, `score`, `tag`, `timestamp` | `give_feedback` |
| `FeedbackUpdatedEvent` | `agent`, `reviewer`, `old_score`, `new_score`, `timestamp` | `update_feedback` |
| `FeedbackRevokedEvent` | `agent`, `reviewer`, `timestamp` | `revoke_feedback` |

**Note**: `FeedbackUpdatedEvent` carries both `old_score` and `new_score`, making it possible to reconstruct reputation history without fetching account state at every point.

---

### Vault Events

11 events covering vault lifecycle, sessions, inscriptions, delegation, and nonce rotation.

| Event | Key Fields | Emitted By |
|-------|-----------|------------|
| `VaultInitializedEvent` | `agent`, `vault`, `wallet`, `timestamp` | `init_vault` |
| `SessionOpenedEvent` | `vault`, `session`, `session_hash`, `timestamp` | `open_session` |
| `MemoryInscribedEvent` | `vault`, `session`, `sequence`, `epoch_index`, `encrypted_data`, `nonce`, `content_hash`, `total_fragments`, `fragment_index`, `compression`, `data_len`, `nonce_version`, `timestamp` | `inscribe_memory`, `inscribe_memory_delegated`, `compact_inscribe` |
| `EpochOpenedEvent` | `session`, `epoch_page`, `epoch_index`, `start_sequence`, `timestamp` | `inscribe_memory` (auto, on epoch boundary) |
| `SessionClosedEvent` | `vault`, `session`, `total_inscriptions`, `total_bytes`, `total_epochs`, `timestamp` | `close_session` |
| `VaultClosedEvent` | `vault`, `agent`, `wallet`, `total_sessions`, `total_inscriptions`, `timestamp` | `close_vault` |
| `SessionPdaClosedEvent` | `vault`, `session`, `total_inscriptions`, `total_bytes`, `timestamp` | `close_session_pda` |
| `EpochPageClosedEvent` | `session`, `epoch_page`, `epoch_index`, `timestamp` | `close_epoch_page` |
| `VaultNonceRotatedEvent` | `vault`, `wallet`, `old_nonce`, `new_nonce`, `nonce_version`, `timestamp` | `rotate_vault_nonce` |
| `DelegateAddedEvent` | `vault`, `delegate`, `permissions`, `expires_at`, `timestamp` | `add_vault_delegate` |
| `DelegateRevokedEvent` | `vault`, `delegate`, `timestamp` | `revoke_vault_delegate` |

**`MemoryInscribedEvent`** is the core data carrier. The `encrypted_data` field contains AES-256-GCM ciphertext. The `nonce` (12 bytes) and `nonce_version` are required for decryption. `compression` values: 0=none, 1=deflate, 2=gzip, 3=brotli.

**`VaultNonceRotatedEvent`** emits the `old_nonce` so clients can still derive keys for historical inscriptions written before the rotation.

---

### Tool Events

8 events for the onchain tool schema registry and session checkpoints.

| Event | Key Fields | Emitted By |
|-------|-----------|------------|
| `ToolPublishedEvent` | `agent`, `tool`, `tool_name`, `protocol_hash`, `version`, `http_method`, `category`, `params_count`, `required_params`, `is_compound`, `timestamp` | `publish_tool` |
| `ToolSchemaInscribedEvent` | `agent`, `tool`, `tool_name`, `schema_type`, `schema_data`, `schema_hash`, `compression`, `version`, `timestamp` | `inscribe_tool_schema` |
| `ToolUpdatedEvent` | `agent`, `tool`, `tool_name`, `old_version`, `new_version`, `timestamp` | `update_tool` |
| `ToolDeactivatedEvent` | `agent`, `tool`, `tool_name`, `timestamp` | `deactivate_tool` |
| `ToolReactivatedEvent` | `agent`, `tool`, `tool_name`, `timestamp` | `reactivate_tool` |
| `ToolClosedEvent` | `agent`, `tool`, `tool_name`, `total_invocations`, `timestamp` | `close_tool` |
| `ToolInvocationReportedEvent` | `agent`, `tool`, `invocations_reported`, `total_invocations`, `timestamp` | `report_tool_invocations` |
| `CheckpointCreatedEvent` | `session`, `checkpoint`, `checkpoint_index`, `merkle_root`, `sequence_at`, `epoch_at`, `timestamp` | `create_session_checkpoint` |

**`ToolSchemaInscribedEvent`** is the schema data carrier. `schema_type`: 0=input, 1=output, 2=description. Verification: `sha256(schema_data) == schema_hash`. `compression`: 0=none, 1=deflate.

---

### Escrow Events

5 events for x402 micropayment settlement.

| Event | Key Fields | Emitted By |
|-------|-----------|------------|
| `EscrowCreatedEvent` | `escrow`, `agent`, `depositor`, `price_per_call`, `max_calls`, `initial_deposit`, `expires_at`, `timestamp` | `create_escrow` |
| `EscrowDepositedEvent` | `escrow`, `depositor`, `amount`, `new_balance`, `timestamp` | `deposit_escrow` |
| `PaymentSettledEvent` | `escrow`, `agent`, `depositor`, `calls_settled`, `amount`, `service_hash`, `total_calls_settled`, `remaining_balance`, `timestamp` | `settle_calls` |
| `EscrowWithdrawnEvent` | `escrow`, `depositor`, `amount`, `remaining_balance`, `timestamp` | `withdraw_escrow` |
| `BatchSettledEvent` | `escrow`, `agent`, `depositor`, `num_settlements`, `total_calls`, `total_amount`, `service_hashes[]`, `calls_per_settlement[]`, `remaining_balance`, `timestamp` | `settle_batch` |

**`PaymentSettledEvent`** serves as a permanent, zero-rent receipt. The `service_hash` is a SHA-256 proof of service ... the agent computes it over the work performed, enabling dispute resolution by third parties.

**`BatchSettledEvent`** preserves individual `service_hashes` and `calls_per_settlement` for granular auditability even when multiple settlements are batched.

---

### Attestation Events

2 events for the web-of-trust system.

| Event | Key Fields | Emitted By |
|-------|-----------|------------|
| `AttestationCreatedEvent` | `agent`, `attester`, `attestation_type`, `expires_at`, `timestamp` | `create_attestation` |
| `AttestationRevokedEvent` | `agent`, `attester`, `attestation_type`, `timestamp` | `revoke_attestation` |

---

### Memory Events

9 events across legacy memory systems and the recommended Memory Ledger.

#### Legacy Memory (gated behind `legacy-memory` feature)

| Event | Key Fields | Emitted By |
|-------|-----------|------------|
| `MemoryStoredEvent` | `agent`, `entry_hash`, `content_type`, `timestamp` | `store_memory` |
| `BufferCreatedEvent` | `session`, `buffer`, `authority`, `page_index`, `timestamp` | `create_buffer` |
| `BufferAppendedEvent` | `session`, `buffer`, `page_index`, `chunk_size`, `total_size`, `num_entries`, `timestamp` | `append_buffer` |
| `DigestPostedEvent` | `session`, `digest`, `content_hash`, `data_size`, `entry_index`, `merkle_root`, `timestamp` | `post_digest` |
| `DigestInscribedEvent` | `session`, `digest`, `entry_index`, `data`, `content_hash`, `data_len`, `merkle_root`, `timestamp` | `inscribe_to_digest` |
| `StorageRefUpdatedEvent` | `session`, `digest`, `storage_ref`, `storage_type`, `timestamp` | `update_digest_storage` |

#### Memory Ledger ([recommended] recommended)

| Event | Key Fields | Emitted By |
|-------|-----------|------------|
| `LedgerEntryEvent` | `session`, `ledger`, `entry_index`, `data`, `content_hash`, `data_len`, `merkle_root`, `timestamp` | `write_ledger` |
| `LedgerSealedEvent` | `session`, `ledger`, `page`, `page_index`, `entries_in_page`, `data_size`, `merkle_root_at_seal`, `timestamp` | `seal_ledger` |

**`LedgerEntryEvent`** carries the raw data in the `data` field ... this is the permanent TX log record. The `merkle_root` is the rolling accumulator after this write, enabling tamper-proof chain verification.

**`LedgerSealedEvent`** records the `merkle_root_at_seal`, letting any verifier confirm that a `LedgerPage` PDA's contents match the merkle state at seal time.

---

## Error Codes

All 91 error codes are defined in the `SapError` enum. Anchor assigns auto-incremented error codes starting at `6000`.
The tables below organize errors by domain.

### Agent Validation (10 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6000 | `NameTooLong` | `name>64` | Agent name exceeds 64 bytes |
| 6001 | `DescriptionTooLong` | `desc>256` | Description exceeds 256 bytes |
| 6002 | `UriTooLong` | `uri>256` | URI exceeds 256 bytes |
| 6003 | `TooManyCapabilities` | `caps>10` | More than 10 capabilities |
| 6004 | `TooManyPricingTiers` | `tiers>5` | More than 5 pricing tiers |
| 6005 | `TooManyProtocols` | `protos>5` | More than 5 protocols |
| 6006 | `TooManyPlugins` | `plugins>5` | More than 5 plugins |
| 6007 | `AlreadyActive` | `already active` | Agent is already active |
| 6008 | `AlreadyInactive` | `already inactive` | Agent is already inactive |
| 6035 | `AgentInactive` | `agent inactive` | Operation requires active agent |

### Deep Validation (15 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6017 | `EmptyName` | `empty name` | Name is zero-length |
| 6018 | `ControlCharInName` | `ctrl char` | Name contains bytes < `0x20` |
| 6019 | `EmptyDescription` | `empty desc` | Description is zero-length |
| 6020 | `AgentIdTooLong` | `agentid>128` | Agent ID exceeds 128 bytes |
| 6021 | `InvalidCapabilityFormat` | `cap format` | Capability not in `domain:action` format |
| 6022 | `DuplicateCapability` | `dup cap` | Duplicate capability ID |
| 6023 | `EmptyTierId` | `empty tier` | Pricing tier ID is empty |
| 6024 | `DuplicateTierId` | `dup tier` | Duplicate pricing tier ID |
| 6025 | `InvalidRateLimit` | `rate=0` | Rate limit is zero |
| 6026 | `SplRequiresTokenMint` | `spl needs mint` | SPL token type without `token_mint` |
| 6027 | `InvalidX402Endpoint` | `x402 https` | x402 endpoint doesn't start with `https://` |
| 6028 | `InvalidVolumeCurve` | `curve order` | Volume curve `after_calls` not strictly ascending |
| 6029 | `TooManyVolumeCurvePoints` | `curve>5` | More than 5 volume curve breakpoints |
| 6030 | `MinPriceExceedsMax` | `min>max price` | `min_price_per_call > max_price_per_call` |
| 6031 | `InvalidUptimePercent` | `uptime 0-100` | Uptime percent > 100 |

### Feedback (5 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6009 | `InvalidFeedbackScore` | `score 0-1000` | Score outside 0...1000 range |
| 6010 | `TagTooLong` | `tag>32` | Tag exceeds 32 bytes |
| 6011 | `SelfReviewNotAllowed` | `self review` | Reviewer wallet == agent owner wallet |
| 6012 | `FeedbackAlreadyRevoked` | `already revoked` | Feedback is already revoked |
| 6047 | `FeedbackNotRevoked` | `not revoked` | Attempting to close non-revoked feedback |

### Indexing (4 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6013 | `CapabilityIndexFull` | `cap idx full` | Capability index has 100 agents |
| 6014 | `ProtocolIndexFull` | `proto idx full` | Protocol index has 100 agents |
| 6015 | `AgentNotInIndex` | `not in idx` | Attempting to remove absent agent |
| 6048 | `IndexNotEmpty` | `idx not empty` | Attempting to close non-empty index |

### Vault (9 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6032 | `SessionClosed` | `session closed` | Writing to a closed session |
| 6033 | `InvalidSequence` | `bad seq` | Sequence doesn't match `session.sequence_counter` |
| 6034 | `InvalidFragmentIndex` | `frag idx` | `fragment_index >= total_fragments` |
| 6037 | `InscriptionTooLarge` | `data>750` | Encrypted data exceeds 750 bytes |
| 6038 | `EmptyInscription` | `empty data` | Zero-length inscription data |
| 6039 | `InvalidTotalFragments` | `frags<1` | `total_fragments < 1` |
| 6040 | `EpochMismatch` | `epoch mismatch` | `epoch_index != session.current_epoch` |
| 6041 | `VaultNotClosed` | `vault open` | Vault has open sessions when closing |
| 6042 | `SessionNotClosed` | `session open` | Session is still open when closing PDA |

### Delegation (2 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6043 | `DelegateExpired` | `delegate expired` | Delegate's `expires_at` has passed |
| 6044 | `InvalidDelegate` | `bad delegate` | Delegate lacks required permission bit |

### Tools (10 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6045 | `ToolNameTooLong` | `tool>32` | Tool name exceeds 32 bytes |
| 6046 | `EmptyToolName` | `empty tool` | Tool name is zero-length |
| 6049 | `InvalidToolNameHash` | `tool hash` | `sha256(tool_name) != tool_name_hash` |
| 6050 | `InvalidToolHttpMethod` | `bad method` | HTTP method not in 0...4 range |
| 6051 | `InvalidToolCategory` | `bad category` | Category not in 0...9 range |
| 6052 | `ToolAlreadyInactive` | `tool inactive` | Tool is already inactive |
| 6053 | `ToolAlreadyActive` | `tool active` | Tool is already active |
| 6054 | `InvalidSchemaHash` | `schema hash` | Schema hash verification failed |
| 6055 | `InvalidSchemaType` | `schema type` | `schema_type` not in 0...2 |
| 6056 | `InvalidCheckpointIndex` | `cp index` | `checkpoint_index != session.total_checkpoints` |

### Escrow (6 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6058 | `InsufficientEscrowBalance` | `low balance` | Balance can't cover settlement amount |
| 6059 | `EscrowMaxCallsExceeded` | `max calls` | Would exceed `max_calls` limit |
| 6060 | `EscrowEmpty` | `escrow empty` | Withdrawal from zero-balance escrow |
| 6061 | `EscrowNotEmpty` | `escrow!=0` | Closing escrow with remaining balance |
| 6062 | `InvalidSettlementCalls` | `calls<1` | `calls_to_settle` is zero |
| 6036 | `EscrowExpired` | `escrow expired` | Escrow has passed `expires_at` |

### Attestation (6 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6063 | `AttestationTypeTooLong` | `atype>32` | Attestation type exceeds 32 bytes |
| 6064 | `EmptyAttestationType` | `empty atype` | Attestation type is zero-length |
| 6065 | `SelfAttestationNotAllowed` | `self attest` | Attester wallet == agent owner wallet |
| 6066 | `AttestationAlreadyRevoked` | `already revoked` | Attestation is already revoked |
| 6067 | `AttestationNotRevoked` | `not revoked` | Closing non-revoked attestation |
| 6069 | `AttestationExpired` | `attest expired` | Attestation has passed `expires_at` |

### Memory (9 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6016 | `ChunkDataTooLarge` | `chunk>900` | Legacy memory chunk exceeds 900 bytes |
| 6070 | `ContentTypeTooLong` | `ctype>max` | Content type string too long |
| 6071 | `IpfsCidTooLong` | `cid>max` | IPFS CID string too long |
| 6072 | `BufferFull` | `buf full` | Buffer would exceed 10 KB max |
| 6073 | `BufferDataTooLarge` | `buf>750` | Single buffer append exceeds 750 bytes |
| 6074 | `Unauthorized` | `unauthorized` | Signer is not the session authority |
| 6075 | `InvalidSession` | `bad session` | Session PDA is invalid or mismatched |
| 6076 | `EmptyDigestHash` | `empty hash` | Content hash is all zeros |
| 6077 | `LedgerDataTooLarge` | `ledger>750` | Ledger write data exceeds 750 bytes |

### Tool Category Index (3 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6068 | `ToolCategoryIndexFull` | `cat idx full` | Category index has 100 tools |
| 6079 | `ToolNotInCategoryIndex` | `not in cat` | Tool not found in category index |
| 6080 | `ToolCategoryMismatch` | `cat mismatch` | Tool's category doesn't match index |

### Batch (2 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6081 | `BatchEmpty` | `batch empty` | Settlements vector is empty |
| 6082 | `BatchTooLarge` | `batch>10` | More than 10 settlements in batch |

### SPL Token (3 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6083 | `SplTokenRequired` | `spl accts` | SPL escrow missing token accounts |
| 6084 | `InvalidTokenAccount` | `bad token` | Token account doesn't match expected mint/owner |
| 6085 | `InvalidTokenProgram` | `bad prog` | Wrong token program passed |

### Safety (4 errors)

| Code | Error | Message | Trigger |
|:----:|-------|---------|---------|
| 6034 | `ArithmeticOverflow` | `overflow` | Integer overflow in counter arithmetic |
| 6057 | `NoFieldsToUpdate` | `no fields` | `update_agent` / `update_tool` called with all `None` |
| 6049 | `SessionStillOpen` | `session open` | Attempting to close PDA while session is open |
| 6078 | `LedgerRingEmpty` | `ring empty` | Attempting to seal an empty ring buffer |

> **Note on error codes**: Anchor assigns error codes sequentially starting at `6000` per the enum declaration order. The codes listed above are derived directly from the `SapError` enum position. Some sharing of numeric codes between logical groups reflects that the same underlying error serves multiple contexts (e.g., `InvalidPluginType` at 6016 is used by both plugin and memory domains).

---

## Event Parsing Example

```typescript
// Listen for real-time events
const listenerId = program.addEventListener("PaymentSettledEvent", (event, slot) => {
  console.log(`Settlement: ${event.callsSettled} calls, ${event.amount} lamports`);
  console.log(`Service hash: ${Buffer.from(event.serviceHash).toString("hex")}`);
  console.log(`Remaining: ${event.remainingBalance} lamports`);
});

// Parse from transaction logs
const tx = await connection.getTransaction(signature, { maxSupportedTransactionVersion: 0 });
const events = program.coder.events.decode(tx.meta.logMessages);
```

## Error Handling Example

```typescript
import { AnchorError } from "@coral-xyz/anchor";

try {
  await program.methods.registerAgent(/* ... */).rpc();
} catch (err) {
  if (err instanceof AnchorError) {
    switch (err.error.errorCode.code) {
      case "EmptyName":
        console.error("Agent name cannot be empty");
        break;
      case "InvalidCapabilityFormat":
        console.error("Capabilities must be in 'domain:action' format");
        break;
      case "DuplicateTierId":
        console.error("Pricing tier IDs must be unique");
        break;
      default:
        console.error(`SAP error: ${err.error.errorMessage}`);
    }
  }
}
```

---

[Previous: 03-accounts.md](03-accounts.md) · [Next: 05-architecture.md](05-architecture.md)
