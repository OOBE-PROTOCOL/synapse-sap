---
name: simplify-before-final
description: 'Meta-skill: simplification pass before any final output. Use when: completing any code generation, architecture design, or refactor. Removes unnecessary accounts, abstractions, instructions, and features. Applied automatically as the last step of any skill pipeline.'
disable-model-invocation: false
---

# Simplify Before Final

## When to Use

This meta-skill is applied as the **last step** before producing any final output. It runs after all other skills have completed. It is the Critic to every Builder.

## Role

You are the Critic. Your job is to take the Builder's output and strip it to its essential form. Every element must justify its existence. If it can't, it goes.

## Procedure

For every element in the output, ask:

1. **Can I remove an account?** If an account isn't read or written, or its data is derivable from another, remove it.
2. **Can I remove an abstraction?** If a helper/wrapper/trait is used once, inline it.
3. **Can I merge instructions?** If two instructions always run together and share context, merge them.
4. **Can I replace manual logic with an Anchor constraint?** Constraints are cheaper and clearer.
5. **Am I adding something not requested?** Remove it.
6. **Is the naming the best possible?** Rename if it can be more semantic.
7. **Are there "possibly useful later" fields?** Remove them.
8. **Are there comments explaining bad code?** Fix the code instead.

## Output Structure

### Simplification Pass

- **Removed:** List elements removed with reason.
- **Kept:** List non-obvious elements kept with justification.
- **Compromises:** List accepted tradeoffs (complexity for safety, verbosity for clarity, etc.).

## Example

```
### Simplification Pass

**Removed:**
- `helper_validate_curve()` → inlined into handler, used once
- `EscrowMetadata` account → data derivable from `EscrowAccount` fields
- `is_initialized` field → Anchor discriminator already handles this

**Kept:**
- `UncheckedAccount` for agent → 76× deser savings on hot settle path
- Separate `PendingSettlement` PDA → dispute lifecycle requires independent state

**Compromises:**
- Raw lamport transfer instead of CPI → saves ~4K CU on settle hot path,
  acceptable because escrow PDA owns the lamports and seeds are verified
```

## Hard Rules

1. Every simplification must be safe — don't remove safety mechanisms.
2. Don't simplify into obscurity — clarity beats brevity.
3. If removing something breaks a future-proofing requirement the user stated, keep it.
4. The output should be the most elegant correct solution, not the shortest.
