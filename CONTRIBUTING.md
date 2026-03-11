# Contributing

Guidelines for contributing to the Synapse Agent Protocol program.

## Prerequisites

| Tool | Version |
|------|---------|
| Rust | 1.93.0 |
| Solana CLI | >= 1.18 |
| Anchor | 0.32.1 |
| Node.js | >= 18 |
| Yarn | >= 1.22 |

## Setup

```bash
git clone https://github.com/OOBE-PROTOCOL/synapse-agent-sap.git
cd synapse-agent-sap
yarn install
anchor build
anchor test
```

## Project Structure

```
programs/synapse-agent-sap/src/
  lib.rs                  Instruction dispatch (72 entries)
  state.rs                22 account structs and enums
  events.rs               45 event definitions
  errors.rs               91 error codes
  validator.rs            Deep validation engine
  instructions/           13 instruction modules
synapse-sap-sdk/          TypeScript SDK
tests/                    Integration tests (10 suites)
docs/                     Protocol documentation
```

## Development Workflow

### Building

```bash
anchor build
```

### Testing

```bash
anchor test
```

All 187 tests must pass before submitting changes.

### Type Checking (SDK)

```bash
cd synapse-sap-sdk
yarn typecheck
yarn build
```

## Adding a New Instruction

1. Add the handler function in `programs/synapse-agent-sap/src/instructions/<module>.rs`.
2. Add the account context struct in the same file.
3. Register the instruction in `lib.rs`.
4. Add any new account types to `state.rs`.
5. Add events to `events.rs` and errors to `errors.rs` as needed.
6. Add integration tests in `tests/`.
7. Update the SDK modules in `synapse-sap-sdk/src/modules/`.
8. Update documentation in `docs/`.
9. Update `CHANGELOG.md`.

## Code Standards

### Rust

- All arithmetic must use checked operations or Anchor's `require!` macro.
- Every instruction must validate inputs through the `validator.rs` engine.
- Account constraints must enforce authorization (no unchecked signers).
- No `unsafe` code.
- Run `cargo clippy` before submitting.

### TypeScript (SDK)

- Strict mode enabled, no `// @ts-ignore`.
- Explicit return types on all public methods.
- JSDoc on all exported functions.
- No `any` except where required by Anchor internals (with comments).

### Naming

| Element | Convention | Example |
|---------|-----------|---------|
| Account structs | PascalCase | `AgentAccount` |
| Instruction handlers | snake_case | `register_agent` |
| Events | PascalCase + Event suffix | `RegisteredEvent` |
| Errors | PascalCase | `AgentNotActive` |
| PDA seeds | snake_case string | `sap_agent` |

## Commit Convention

This project follows [Conventional Commits](https://www.conventionalcommits.org/) v1.0.0:

```
<type>(<scope>): <description>
```

### Types

| Type | Usage |
|------|-------|
| `feat` | New feature or instruction |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `refactor` | Code restructuring |
| `test` | Test additions or fixes |
| `build` | Build system or dependency changes |
| `chore` | Maintenance tasks |

### Scopes

Use module names: `agent`, `vault`, `escrow`, `ledger`, `tools`, `feedback`,
`indexing`, `attestation`, `sdk`, `docs`.

### Examples

```
feat(ledger): add ring buffer compaction
fix(escrow): handle zero-balance SPL withdrawal
docs(security): update threat model
test(vault): add delegation edge cases
```

## Pull Request Process

1. Create a feature branch from `main`.
2. Implement changes following the standards above.
3. Ensure `anchor build` and `anchor test` pass.
4. Update `CHANGELOG.md` under `[Unreleased]`.
5. Open a pull request targeting `main`.
6. Address review feedback.
7. Squash and merge.

## Release Process

1. Move `[Unreleased]` entries in `CHANGELOG.md` to a new version section.
2. Tag the release: `git tag v<version>`.
3. Push: `git push origin main --tags`.
4. Deploy if applicable: `solana program deploy target/deploy/synapse_agent_sap.so`.

## Security

Report vulnerabilities to security@oobeprotocol.ai. Do not open public issues for
security-related findings.

## License

MIT
