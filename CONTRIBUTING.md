# Contributing to Constella

Thanks for your interest in Constella — open-source, modular compliance infrastructure for Stellar RWA tokens. This guide explains the architecture and, most importantly, **how to write a new compliance module**.

## Architecture in one paragraph

Every regulated transfer is checked by two orthogonal layers: **identity** ("is this address eligible?") and **compliance** ("does this transfer satisfy the rules?"). Constella builds the compliance layer: a **dispatcher** (`crates/compliance`) holds a per-hook registry of **modules**, AND-combines their pre-checks, and fans out post-events to the stateful ones. A module is a separate Soroban contract that implements the shared ABI in `crates/module-interface`. See [`README.md`](README.md) and [`docs/architecture.md`](docs/architecture.md) for the full picture.

## The module ABI

A module implements the `ComplianceModule` trait (from `crates/module-interface`). Implement **only the hooks your rule needs**:

```rust
fn can_transfer(env: Env, from: Address, to: Address, amount: i128, token: Address) -> bool; // pre-check
fn can_create(env: Env, to: Address, amount: i128, token: Address) -> bool;                  // pre-check (mint)
fn transferred(env: Env, from: Address, to: Address, amount: i128, token: Address);          // post-event
fn created(env: Env, to: Address, amount: i128, token: Address);                             // post-event (mint)
fn destroyed(env: Env, from: Address, amount: i128, token: Address);                         // post-event (burn)
```

- **Pre-checks** (`can_*`) return `bool`. The dispatcher AND-combines every registered module's result; any `false` denies the operation.
- **Post-events** (`transferred` / `created` / `destroyed`) let stateful modules update their bookkeeping after a transfer settles.

The hook set is `ComplianceHook = { CanTransfer, CanCreate, Transferred, Created, Destroyed }`. A module is registered per hook via `compliance.add_module_to(hook, module)`.

## ⚠️ The one rule you must not break: no re-entrancy

A module **cannot call back into the token** mid-transfer — the Soroban host forbids re-entering a contract already on the call stack. So **balance-dependent modules must not read the token's balance**. Instead, maintain your **own balance mirror** updated from the post-event hooks (`created` / `transferred` / `destroyed`). See `crates/module-max-balance` and `crates/module-max-holders` for the pattern. A module that uses a mirror **must** be registered on the post-event hooks from genesis, or its mirror will drift.

Modules that only read the **identity layer** (e.g. `country_of`, `is_verified`) are fine — that's a different contract, not re-entrancy.

## Writing a new module — step by step

1. **Create the crate:** `crates/module-<your-rule>/` with `Cargo.toml` inheriting the workspace (`soroban-sdk`, `crate-type = ["cdylib"]`) and a `module-interface` path dependency.
2. **Implement `__constructor`** to store config/admin (follow an existing module, e.g. `module-max-balance`).
3. **Implement the hooks** your rule needs. Keep pre-checks pure/cheap; do bookkeeping in post-events.
4. **Expose a read accessor** (e.g. `max()`, `allowed()`) so the UI / Ambassador evidence can inspect the rule on-chain.
5. **Add tests** in `src/test.rs` (unit) and, if it touches transfers end-to-end, extend the `demo-token` integration test.
6. **Document it:** add a `README.md` to the crate (rule, hooks used, config, stateful? identity-dependent?).
7. **Wire it in the deploy script** (`scripts/deploy-testnet.sh`) on the correct hooks.

## Build, test, lint

```bash
stellar contract build           # build all contracts to wasm
cargo test                       # run the workspace test suite (host)
cargo clippy --workspace         # lints
cargo fmt                        # format
```

CI (GitHub Actions, `.github/workflows/ci.yml`) runs tests + coverage and builds every contract to `wasm32v1-none` on each PR. Keep it green.

## Pull-request flow

We use GitHub Flow: branch off `main` → open a PR → squash-merge. Keep PRs focused; one module or one concern per PR. Make sure CI is green and new code has tests before requesting merge.

## License

By contributing you agree your contributions are licensed under **Apache-2.0** (see [`LICENSE`](LICENSE)).
