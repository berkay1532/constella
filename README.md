# Constella

**Open-source, modular compliance infrastructure for Stellar RWA tokens (Soroban).**

Constella is a library of audit-ready, reusable **compliance modules** plus a **standard module interface** that plug into OpenZeppelin's Soroban RWA compliance engine. Issuers compose ready-made rules (holder caps, lock-ups, concentration limits, country restrictions, …) instead of hand-writing them from scratch.

> Modules = stars forming a compliance constellation. ✨

## What it is / isn't

- ✅ **Compliance module library + standard interface** (infrastructure / pick-and-shovel).
- ❌ Not an RWA launchpad/issuance platform (today) — though the roadmap evolves toward one.
- ❌ Not an asset issuer.

## How it fits

```
TRANSFER
  ├── Identity layer    → OZ IdentityVerifier + provider   (we consume, pluggable)
  └── Compliance layer  → OZ Compliance engine + CONSTELLA MODULES   (we build)
```

OpenZeppelin provides the RWA token + compliance dispatcher (engine). Constella provides the **modules**, the **standard module trait**, and (for the demo) a mock identity layer.

## Status

🟢 **MVP built & verified.** Contracts compile to wasm, `cargo test` is green, the full
stack is deployed and exercised live on Stellar testnet (real pass + revert), and a web
demo simulates compliance gating in the browser.

- Product requirements: [`docs/PRD-Constella.md`](docs/PRD-Constella.md)
- Architecture: [`docs/architecture.md`](docs/architecture.md)
- Decisions log: [`docs/DECISIONS.md`](docs/DECISIONS.md)

## Layout

```
crates/        Soroban contracts (Rust)
  module-interface/          standard module ABI + clients
  compliance/                dispatcher engine (mirrors OZ surface)
  module-max-holders/        \
  module-lockup/              } the 4 MVP compliance modules
  module-max-balance/        /
  module-country-restrict/  /
  identity-mock/             attestor stand-in (IdentityProvider)
  demo-token/                minimal SEP-41 permissioned token + integration test
  zk-verifier/               Groth16 / BLS12-381 verifier (Phase 2)
  module-identity-zk/        ZK identity provider — prove country ∈ allowed, hidden (Phase 2)
zk/            ZK circuit (country eligibility) + proof artifacts (Phase 2)
scripts/       testnet deploy + live demo (deploy-testnet.sh)
web/           React launch-taste demo (live pass/revert simulation)
docs/          PRD, architecture, decisions
```

## Quickstart

```bash
# build + test the contracts
stellar contract build
cargo test

# deploy to testnet and run a real pass/revert demo
bash scripts/deploy-testnet.sh

# run the web demo
cd web && npm install && npm run dev
```

## Roadmap (high level)

1. **MVP** — module trait + 4 modules (MaxHolders, Lockup, MaxBalance, CountryRestrict) + mock identity + a thin "launch-taste" demo (Web UI) with real testnet transactions.
2. **ZK leapfrog** — ZK-private eligibility (`module-identity-zk`): prove eligibility without revealing identity/attributes.
3. **Ecosystem/product** — module registry, deployment factory, hosted issuer console (launchpad), premium modules.

## License

TBD (to be aligned with OpenZeppelin `stellar-contracts`).
