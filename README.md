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

🚧 Early / pre-MVP. Design phase.

- Product requirements: [`docs/PRD-Constella.md`](docs/PRD-Constella.md)
- Architecture (in progress): [`docs/architecture.md`](docs/architecture.md)

## Roadmap (high level)

1. **MVP** — module trait + 4 modules (MaxHolders, Lockup, MaxBalance, CountryRestrict) + mock identity + a thin "launch-taste" demo (Web UI) with real testnet transactions.
2. **ZK leapfrog** — ZK-private eligibility (`module-identity-zk`): prove eligibility without revealing identity/attributes.
3. **Ecosystem/product** — module registry, deployment factory, hosted issuer console (launchpad), premium modules.

## License

TBD (to be aligned with OpenZeppelin `stellar-contracts`).
