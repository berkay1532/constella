# PRD — Constella

> **Constella** — open-source, modular compliance infrastructure for Stellar RWA tokens (with a roadmap evolving toward a launchpad). Modules = stars forming a compliance constellation.
> **Status:** Draft v1.0 · **Date:** 2026-06-02 · **Owner:** Berkay · **Author:** Nicole (PM)

---

## 1. One-liner

**An open-source, audit-ready, reusable library of compliance modules + a standard module interface that plug into OpenZeppelin's Soroban RWA compliance engine.** Issuers compose ready-made modules instead of hand-writing compliance rules from scratch.

---

## 2. What it is, what it isn't (positioning)

- ✅ **Product = compliance module library + standard interface (infrastructure / pick-and-shovel).**
- ❌ NOT an RWA launchpad / issuance platform (today).
- ❌ NOT us issuing an RWA as a regulated issuer.
- 🎯 **The demo gives a "taste" of the launchpad** (a thin launch flow) — because that's the direction SCF investment points toward. But we are not building the full launchpad; that's the roadmap (Phase 2–3).

---

## 3. Problem (two layers)

**Layer 1 — Today's blocker (what the MVP solves):**
Issuing a regulated real-world asset (RWA) on Stellar requires enforcing transfer rules (allowed country, holder cap, lock-up, concentration limit…) on-chain. OpenZeppelin provides the **RWA token and the compliance engine (dispatcher)** — **but not the rules themselves (the modules), nor a standard interface the modules must implement.** As a result, every issuer must write each rule **from scratch, one by one, unaudited.** This is the silent blocker to compliant RWA issuance on Stellar.

**Layer 2 — The deeper problem (what the ZK roadmap solves):**
Identity-based rules (country/accreditation) require writing investor attributes on-chain **in cleartext** → this **leaks PII and exposes the investor set.** Even EVM's ERC-3643 hasn't solved this (the GDPR pain).

**Evidence:**
- SEP-57: Draft v0.2.0 (current, in flux). OZ `stellar-tokens` v0.7.1: provides the token + compliance *framework*, but no ready-made module *library*.
- 728-project ecosystem DB: live issuers actually using SEP-57/T-REX ≈ 0; the large Stellar RWA TVL runs on the older classic-asset model → supports the blocker hypothesis.
- On EVM, the ERC-3643 ecosystem enabled ~$28–32B of tokenization (Tokeny, marketing-grade) — the value of modular compliance is proven.

---

## 4. Target users

| Persona | Need |
|---|---|
| **RWA issuer** | Ship a compliant token fast, with audited rules |
| **Tokenization platform / launchpad** | Ready-made, composable compliance modules to offer clients |
| **Soroban developer** | Plug rules into the OZ RWA token without writing them from scratch |

**Horizontal:** not specific to geography or asset type.

---

## 5. Architecture boundary (two layers — the ERC-3643 model)

```
TRANSFER
   ├── LAYER 1: IDENTITY   → "is this address verified / eligible?"
   │     = OZ IdentityVerifier + identity provider   ◄── WE CONSUME (mock/pluggable), don't build
   └── LAYER 2: COMPLIANCE → "does this transfer satisfy the rules?"
         = OZ Compliance engine + ► OUR MODULES ◄    ◄── WE BUILD
```

- **OZ provides:** RWA token, Compliance dispatcher (engine), IdentityVerifier interface, ComplianceHook enum.
- **We write:** the standard module trait + the module library + a mock identity layer + the demo.
- **Trust model:** self-contained modules are trustless (on-chain state only); the identity module relies on an attestor (a KYC provider in production, us as issuer in the demo). This is the correct model for compliance.
- **Modules don't care HOW identity is verified** — they ask the identity layer "is this address eligible for attribute X." The answer can come from a cleartext registry or from ZK. That's why adding ZK doesn't break the modules.

---

## 6. MVP scope (3–5 months, solo)

**Modules (4):**

| Module | Rule | Type |
|---|---|---|
| **MaxHolders** | Cap on the number of holders | Self-contained (stateful: counter) |
| **Lockup** | No transfers for a set period | Self-contained (stateful: time) |
| **MaxBalance** | Per-address concentration limit | Self-contained (stateless) |
| **CountryRestrict** | Allowed countries only | Identity-dependent |

**Supporting pieces:**
- **Standard module interface (trait)** — the core contribution.
- **Mock identity/attribute registry** — for CountryRestrict (pluggable; idOS/attest in production).
- **Thin "launch-taste" demo flow** — deploy a token + select & plug modules + a real testnet transfer that passes/reverts.

**Design balance:** 3 self-contained (zero trust dependency, demo guarantee) + 1 identity (RWA-compliance feel + identity-layer integration); a mix of stateless + stateful (proves the standard interface handles every scenario).

**Out of scope (later phases):** building an identity/KYC network, the full deployment factory, the module registry, a hosted console/launchpad, the ZK layer.

---

## 7. Success metrics (measurable)

- ≥ 4 production-quality, tested modules plugging into the OZ Compliance hooks.
- The standard module trait defined and documented.
- End-to-end testnet demo: a compliant token + ≥ 2 modules; a non-compliant transfer **reverts**, a compliant one **passes** (visible in an explorer).
- The thin launch flow works (token + module selection in one flow).
- Open-source repo + documentation; at least 1 external developer tries a module and gives feedback.

**Counter-metric:** we don't issue our own asset; we don't become a KYC provider/regulated intermediary.

---

## 8. Demand evidence + riskiest assumption + validation

**Demand evidence (for mentor/SCF — the answer to "are you building for zero demand?"):**
- **The category carries real institutional volume on other chains:** ERC-3643 has **149 permissioned token contracts** (Polygon 83, Avalanche 34, Ethereum 32) + **~12,500 whitelisted investors** (on-chain verified, Dune/QualitaX). Institutional roster: Apex, Invesco, Archax, Bitstamp. *(Note: the "$28–32B tokenized" figure is issuer-reported, not on-chain verifiable — use 149/12.5K as the hard numbers.)*
- **The restriction NEED is already proven on Stellar today:** non-stablecoin RWA is **$2B+**, with **+91% growth in Q1 2026** (Messari). BENJI (Franklin Templeton), Ondo, WisdomTree, Spiko. And these **already enforce holder restrictions today** — permissioned-wallet/whitelist + Stellar-native `AUTH_REQUIRED`/`AUTH_REVOCABLE` + clawback + SEP-8 (Final). The need is proven by behavior; we make it modular/standard/audited.
- **A dated, named wave is coming:** **DTCC H1 2027** — Russell 1000 stocks, ETFs, Treasuries; 50+ institutions; Dec 2025 SEC No-Action Letter. DTCC chose Stellar **for its compliance design** (protocol-level transfer restrictions/clawback/identity, via Securrency).
- ⚠️ **Don't overclaim:** it's not confirmed that DTCC will use SEP-57/a modular layer (inference). SEP-57 adoption is ~0 today; existing issuers use bespoke compliance → this is our real "pre-adoption" risk.

**Riskiest assumption (not technical — demand):** *"Issuers will use standard modules instead of bespoke ones / Stellar RWA will converge on this layer."*

**Validation (before build, ~1 week):**
1. **Ask the OZ RWA / ERC-3643 Association team:** "Which compliance modules are most needed, what's missing?" → finalize the first modules accordingly.
2. **Ask 3–5 potential issuers/tokenization platforms:** "If audited, ready-made compliance modules existed, would you use them?"

If both give a green light, build with full confidence.

---

## 9. Roadmap

| Phase | Content | Position |
|---|---|---|
| **1 — MVP** | Trait + 4 modules + mock identity + launch-taste demo (Web UI) | Public good · SCF |
| **2 — ZK leapfrog** | **ZK-private eligibility** (`module-identity-zk`): the investor proves eligibility (country/accreditation) **without revealing it**; integrated into the demo and shown. *In the demo, attestor = us, we control the credential format; production format adoption is validated separately.* | Differentiator |
| **3 — Ecosystem/product** | Module registry + one-tx deployment factory + **hosted issuer console (launchpad)** + premium modules + compliance-as-a-service | Product/business |

---

## 10. GTM / Business model (open core)

- **Model:** open core is free (adoption + SCF + credibility); the layers issuers pay for are commercial.
- **Proof (analog):** Tokeny open-sourced ERC-3643 → built the commercial T-REX Platform → Apex Group took a majority stake in 2025. The Stellar version of this project can follow the same path.
- **Revenue (Phase 2–3):** hosted issuer console/launchpad (SaaS), compliance-as-a-service (jurisdiction rule-packs), premium + ZK-private modules, enterprise support/audits.
- **Strategic insight:** the issuers who use the modules in Phase 1 become the distribution channel and moat for the console upsell in Phase 3. Open core = the product's GTM.

---

## 11. SCF fit

| Criterion | Match |
|---|---|
| **Ecosystem value** | Horizontal infrastructure every RWA issuer would use; aligned with SDF's ERC-3643 Association membership; unblocks the bottleneck. |
| **Technical feasibility** | Porting EVM-proven modules to Soroban; bounded; real transactions on testnet. Solo-feasible. |
| **Roadmap + team** | Clear MVP → ZK → registry/factory/console; open-source ecosystem contribution. Build Award *or* Public Goods track fits. |

---

## 12. Risks

| Risk | Mitigation |
|---|---|
| Chicken-and-egg (SEP-57 adoption ~0) | Design modules to work for any Soroban permissioned token; be the one who unblocks; validate demand |
| OZ/SDF builds it themselves | Open source + early reference = complementary; contributor position in the Association |
| OZ API pre-1.0, in flux | Pin the version (`=0.7.1`); track breaking changes |
| Product-layer competition (Tokeny multi-chain) | Capture "Stellar compliance layer" mindshare in Phase 1 |
| ZK-layer adoption (credential format) | **Phase 2** (right after MVP); in the demo we control the format (feasible), production credential-adoption validated separately |

---

## 13. Open decisions

- ~~Name~~ → **Constella** ✅
- ~~Demo surface~~ → **Web UI** ✅ (launch-taste flow)
- **License:** OZ Apache/GPL compatibility (to be finalized).
- **SCF track:** Build Award (larger grant) or Public Goods? *(after hackathon/mentor)*
- **Launchpad vs. staying a public good:** to be evaluated with mentors after the hackathon.

---

## 14. Next steps

1. Mentor meeting (this PRD + the GTM story).
2. Validation (§8): OZ/Association + 3–5 issuers.
3. Architecture doc (Tyler / `bmad-create-architecture`): module trait signature, storage layout, identity interface, test strategy.
4. Epics & stories → build.

---

*This PRD emerged from discovery (deep iterative analysis in lieu of live user interviews). It should be updated with mentor + validation feedback.*
