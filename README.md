# Constella

**Open-source, modular compliance infrastructure for Stellar RWA tokens (Soroban) — with a zero-knowledge privacy layer.**

[![CI](https://github.com/berkay1532/constella/actions/workflows/ci.yml/badge.svg)](https://github.com/berkay1532/constella/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

Constella is a library of audit-ready, reusable **compliance modules** plus a **standard module interface** that plug into an OpenZeppelin/ERC-3643-style compliance engine. Issuers compose ready-made rules (holder caps, lock-ups, concentration limits, country restrictions, …) instead of hand-writing them — and can upgrade the identity check to a **zero-knowledge** one that proves an investor is eligible *without revealing their country*.

> Modules = stars forming a compliance constellation. ✨

- **Status:** MVP built & verified — `cargo test` green, all contracts build to wasm, the full stack runs live on Stellar testnet (real pass/revert), and a React demo signs real transfers with Freighter. Phase 2 (ZK) is implemented and wired into the demo.
- **Repo:** `github.com/berkay1532/constella`
- **Docs:** [PRD](docs/PRD-Constella.md) · [Architecture](docs/architecture.md) · [Decisions log](docs/DECISIONS.md) · [ZK notes](zk/README.md) · [Contributing](CONTRIBUTING.md) · [License](LICENSE)

---

## 1. The idea in one picture

Every regulated tokenized asset must enforce *who can hold/receive it* (jurisdiction, KYC, holder caps, lock-ups). ERC-3643/T-REX (and Stellar's draft SEP-57) model this as **two orthogonal checks on every transfer**:

```
           transfer(from, to, amount)
                      │
   ┌──────────────────┴───────────────────┐
   ▼                                       ▼
LAYER 1 — IDENTITY                 LAYER 2 — COMPLIANCE
"is this address eligible?"        "does this transfer satisfy the rules?"
   │                                       │
   ├─ identity-mock (cleartext)            ├─ MaxHolders     ┐
   └─ module-identity-zk (ZK, private)     ├─ Lockup         │ pluggable
                                           ├─ MaxBalance     │ modules
                                           ├─ CountryRestrict│ (Constella)
                                           └─ ZkEligibility  ┘
```

Constella **builds Layer 2** (the compliance modules + the standard interface + the dispatcher) and **consumes Layer 1** (identity) through a small interface, so the identity check can be a cleartext registry *or* a zero-knowledge proof — the modules don't care which.

---

## 2. What we built vs. what we reuse

| Piece | Source |
|---|---|
| Compliance **dispatcher** (engine), **demo token**, **module library**, **standard module ABI** | **Constella** (this repo). Mirrors the OpenZeppelin RWA / ERC-3643 hook surface so modules stay portable, but is a self-contained reference impl (OZ doesn't publish a module ABI — see [DECISIONS D3](docs/DECISIONS.md)). |
| Design model (T-REX / ERC-3643, two-layer identity+compliance) | OpenZeppelin / SEP-57 (we follow the architecture). |
| BLS12-381 pairing crypto on-chain | `soroban-sdk` native `env.crypto().bls12_381()` (CAP-0059). |
| Groth16 verifier pattern | Adapted from `stellar/soroban-examples/groth16_verifier`. |
| ZK circuit + proving | `circom` + `snarkjs` + `circomlib` (Poseidon), BLS12-381. |
| snarkjs-JSON → soroban byte encoding | `arkworks` (off-chain, in tests + `tools/zk-encode`). |
| Wallet, RPC, dApp | `@stellar/stellar-sdk`, Freighter, React + Vite. |

---

## 3. Repository layout

```
crates/                         Soroban contracts (Rust)
  module-interface/             standard module ABI: ComplianceModule / TokenRead / IdentityProvider
                                clients, ComplianceHook, ComplianceError
  compliance/                   the dispatcher (engine): per-hook module registry,
                                AND-combined pre-checks, fan-out post-events
  module-max-holders/           ┐
  module-lockup/                │ the 4 self-contained / cleartext compliance modules
  module-max-balance/           │
  module-country-restrict/      ┘ (CountryRestrict reads the identity layer)
  identity-mock/                cleartext attestor (country_of / is_verified)
  demo-token/                   minimal SEP-41 permissioned token (calls compliance) + integration test
  zk-verifier/                  Groth16 / BLS12-381 verifier (Phase 2)
  module-identity-zk/           ZK identity provider — prove country ∈ allowed, hidden (Phase 2)
  module-zk-eligibility/        compliance module gating on the ZK is_verified flag (Phase 2)
zk/                             ZK circuit (country_eligibility.circom) + build.sh + proof artifacts
tools/zk-encode/                Rust: snarkjs JSON → soroban byte format (for CLI submission)
scripts/deploy-testnet.sh       deploy everything + wire + live pass/revert; writes deployed.testnet.json
web/                            React + Vite demo (Freighter, live transfers, ZK panels)
docs/                           PRD, architecture, decisions
```

---

## 4. The compliance engine (Layer 2)

### 4.1 The module ABI (`module-interface`)

A compliance module is a separate contract the dispatcher calls. It may implement any of these hooks (and only the ones it needs):

```
can_transfer(from, to, amount, token) -> bool     // pre-check
can_create(to, amount, token)         -> bool     // pre-check (mint)
transferred(from, to, amount, token)              // post-event
created(to, amount, token)                         // post-event (mint)
destroyed(from, amount, token)                     // post-event (burn)
```

`ComplianceHook` = `{ CanTransfer, CanCreate, Transferred, Created, Destroyed }`. This mirrors the OZ RWA surface so the same modules are portable to OZ's dispatcher.

### 4.2 The dispatcher (`compliance`)

Stores `Map<ComplianceHook, Vec<Address>>` of registered module addresses and runs them:

- `can_transfer` / `can_create`: call every registered module's pre-check and **AND-combine** — any `false` ⇒ deny.
- `transferred` / `created` / `destroyed`: **fan out** to every registered module so stateful ones update their bookkeeping.
- `add_module_to(hook, module)` / `remove_module_from(hook, module)` (admin).

### 4.3 The four cleartext modules

| Module | Rule | Type | How |
|---|---|---|---|
| **MaxHolders** | ≤ N distinct holders | stateful | keeps its **own balance mirror** + holder count from the post-event stream |
| **Lockup** | no transfer for T seconds after acquiring | stateful (time) | records acquisition ledger time per holder; gate = `now ≥ acquired + T` |
| **MaxBalance** | ≤ cap per holder | stateful mirror | own balance mirror; gate = `balance(to) + amount ≤ cap` |
| **CountryRestrict** | recipient's country ∈ allowed | identity-dependent | reads `country_of(to)` from the **identity layer** |

> **Soroban re-entrancy:** a module **cannot call back into the token** mid-transfer (the host forbids re-entering a contract already on the call stack). So balance-dependent modules (MaxHolders, MaxBalance) **never read the token** — they maintain their own balance mirror from `created`/`transferred`/`destroyed`. This is why they must be registered on the post-event hooks from genesis.

---

## 5. The transfer / mint flow (step by step)

`demo-token` is a minimal SEP-41-style permissioned token. Every mutating call goes through the dispatcher.

### Mint `mint(to, amount)`

```
1. admin.require_auth()
2. token = current_contract_address()
3. compliance.can_create(to, amount, token)
        └─ dispatcher → for each CanCreate module: can_create(...) ; AND-combine
        └─ if false → panic ComplianceError::Denied  (transaction reverts)
4. balance[to] += amount ;  supply += amount
5. compliance.created(to, amount, token)
        └─ dispatcher → for each Created module: created(...)
           • MaxHolders.created → mirror[to]+=amount ; if 0→+ holders++
           • Lockup.created      → acquired[to] = ledger.timestamp
           • MaxBalance.created  → mirror[to]+=amount
```

### Transfer `transfer(from, to, amount)`

```
1. from.require_auth()
2. token = current_contract_address()
3. compliance.can_transfer(from, to, amount, token)
        └─ dispatcher → for each CanTransfer module (AND-combine):
           • MaxHolders     → is `to` new AND holders==max ? deny
           • Lockup         → ledger.timestamp ≥ acquired[from] + T ?
           • MaxBalance     → mirror[to] + amount ≤ cap ?
           • CountryRestrict→ country_of(to) ∈ allowed ?     (reads identity layer)
        └─ if any false → panic Denied (revert)
4. balance[from] -= amount ;  balance[to] += amount
5. compliance.transferred(from, to, amount, token)
        └─ stateful modules update their mirrors / holder count
```

The web demo never makes you sign a doomed transfer: the wallet **simulates** (`prepareTransaction`) first — which runs the real on-chain compliance check — and only asks Freighter to sign if it passes. A denial therefore happens *before signing* (and Soroban can't even build a submittable tx without a successful simulation).

---

## 6. The zero-knowledge layer (Phase 2)

Goal: prove an investor's **country is in the allowed set without revealing the country**, and gate transfers on that — so a non-eligible recipient is rejected *without their country ever appearing on-chain*.

### 6.1 The circuit (`zk/country_eligibility.circom`)

- **Private:** `country`, `secret`. **Public:** `commitment` (circuit output), `allowed[]`.
- Proves: (1) `commitment == Poseidon(country, secret)` — binds to an issuer-registered commitment; (2) `country ∈ allowed` via `∏(country − allowed[i]) == 0` (no hash needed for membership).
- The country never leaves the prover; the public signals are only `[commitment, allowed…]`.

### 6.2 Proving → on-chain verification

```
zk/build.sh:  circom (--prime bls12381) + snarkjs (powersoftau → groth16 setup → prove)
              → zk/data/{proof,public,verification_key}.json   (a real proof)
                      │
  off-chain encode (arkworks): snarkjs decimal coords → uncompressed bytes
   • tools/zk-encode → CLI-ready hex (G1 = 96B, G2 = 192B; signals = decimal u256)
   • the Rust tests do the same via include_str! + serde_json
                      │
  on-chain (crates/zk-verifier): verify_proof(vk, proof, pub_signals)
   • uses env.crypto().bls12_381().pairing_check  → e(-A,B)·e(α,β)·e(vk_x,γ)·e(C,δ)==1
   • ~40M instructions (~40% of the 100M tx budget)
```

### 6.3 The ZK identity provider (`module-identity-zk`)

Same `IdentityProvider` surface as the mock, but private:

```
register_commitment(account, commitment)        // issuer registers the public commitment
prove_eligibility(account, commitment, proof)    // builds pub_signals = [commitment, allowed…]
   └─ Groth16VerifierClient(verifier).verify_proof(vk, proof, signals)   (cross-contract)
   └─ on success: eligible[account] = true
is_verified(account) -> bool                      // the flag compliance reads
country_of(account)  -> None                      // the country is private — never stored
```

### 6.4 The ZK gate (`module-zk-eligibility`)

A compliance module whose `can_transfer/can_create` check `is_verified(to)` on the ZK provider — **a boolean, no country read**. Registered on a second token (`zk_token`) in the demo. A disallowed recipient is simply "not eligible"; their country never appears on-chain. This is the privacy win over `CountryRestrict`.

> **What's public vs private here is by design.** The *eligible / not-eligible* boolean is **intentionally public** — that's exactly what a compliance check exists to expose. What ZK hides is the **sensitive attribute** (the country / identity). So Constella reveals the minimum a regulator needs (eligibility) and nothing more (the country).

---

## 7. The web demo (`web/`)

React + Vite + `@stellar/stellar-sdk` + Freighter, reading `web/src/deployed.testnet.json` (contracts + accounts + `zk` section). Two stories, side by side:

**A — The problem (cleartext compliance).** Connect Freighter → *Prepare* (dev `/api/bootstrap`: fund + `set_country` + mint) → send to **Bob** (passes) / **Carol** (denied — and the denial reveals `country = TR`). All real, Freighter-signed transactions; balances update in the holders table.

**B — The privacy fix (ZK).** *Prove eligibility (ZK)* (dev `/api/zk-prove` registers the commitment + submits a **real Groth16 proof on-chain**) → `is_verified = true`, `country_of = none`. Then *Get ZK-gated tokens* (`/api/zk-mint`) and send `zk_token` to **Dave** (ZK-eligible → passes) / **Carol** (not eligible → denied — **country never read or revealed**).

The dev API endpoints (`/api/*`) run the local `stellar` CLI as the **deployer/admin** for setup steps; the admin secret never enters the frontend bundle, and they exist only under `npm run dev`. The actual transfers the jury cares about are signed by **your** Freighter wallet. (A read-only simulation path also exists for showing the gate without signing.)

---

## 8. How it all connects (deploy)

`scripts/deploy-testnet.sh` is the single source of truth for a live environment. It:

1. funds `deployer` / `alice` / `bob` (friendbot), generates `carol`;
2. deploys identity-mock, the dispatcher, the 4 modules, the demo token; wires modules to hooks; sets cleartext identities;
3. runs a real **mint + pass + revert** to prove the cleartext path;
4. deploys `zk-verifier` + `module-identity-zk`, runs `tools/zk-encode`, and `set_policy(vk, allowed)`;
5. deploys `module-zk-eligibility` + `zk_compliance` + `zk_token`, and proves an eligible recipient **dave**;
6. writes every address (incl. the `zk` section: verifier, identityZk, commitment, proof, zkToken, dave) to `scripts/deployed.testnet.json`.

`deployed.testnet.json` is what both the web app and the dev API read.

### 8.1 Deployed on testnet (live)

Network: **testnet** · RPC `https://soroban-testnet.stellar.org` · passphrase `Test SDF Network ; September 2015`. Every contract below is live and explorable — click an address.

**Core compliance stack (cleartext path):**

| Contract | Address | What it does |
|---|---|---|
| **demo-token** | [`CDWDVUSP…ALKH4ISP`](https://stellar.expert/explorer/testnet/contract/CDWDVUSP5KJWS562PPZY2AJZKZTGDYXTEL34LLB5DEBCGQOKALKH4ISP) | SEP-41-style permissioned RWA token; routes every mint/transfer through the dispatcher |
| **compliance** | [`CB4PQFPC…NBGSBB`](https://stellar.expert/explorer/testnet/contract/CB4PQFPCVFAZHU6LO3Y2B3QQPEK25GUJGDCR6H3QPWJEERJ7GWNBGSBB) | the dispatcher (engine): per-hook module registry, AND-combined pre-checks, fan-out post-events |
| **identity-mock** | [`CDCTU7NK…I3SHVLZD`](https://stellar.expert/explorer/testnet/contract/CDCTU7NKRB7A6NDZPMZZIQN7DVXUHRDO6M5IHQ5OAJYUHS3UI3SHVLZD) | cleartext attestor — `country_of` / `is_verified` (Layer 1 identity) |
| **module-country-restrict** | [`CCWBDSDD…334VU3FA`](https://stellar.expert/explorer/testnet/contract/CCWBDSDD7MSUSRUVJQKRJVXEG67AVDZMTJ7KTFFX5TSXF72F334VU3FA) | recipient's country ∈ allowed (US, DE); reads the identity layer |
| **module-max-holders** | [`CAXX3AHY…24TDNHFL`](https://stellar.expert/explorer/testnet/contract/CAXX3AHYYTFAEOEPDKARXKKI6I2J7NA3ESWEL5GAYHTJUXQK24TDNHFL) | ≤ 5 distinct holders; self-tracks a balance mirror from the event stream |
| **module-max-balance** | [`CDXG5N5V…MZCI5GA7`](https://stellar.expert/explorer/testnet/contract/CDXG5N5VCU5PWNAUNBBDUSMOFZOXWZYYTNRR7FZ5KOY6CX44MZCI5GA7) | ≤ cap per holder; self-tracks a balance mirror |
| **module-lockup** | [`CADJ6CKD…VWVSYNAL`](https://stellar.expert/explorer/testnet/contract/CADJ6CKDBZPQ5EM2JCBQK5CWYY2H6QQ3IIFKI4P6NK3OMNYXVWVSYNAL) | no transfer for T seconds after acquiring (T = 0 in the demo) |

**Zero-knowledge layer (Phase 2):**

| Contract | Address | What it does |
|---|---|---|
| **zk-verifier** | [`CCACFVOM…XWOMCAD`](https://stellar.expert/explorer/testnet/contract/CCACFVOMNQRKGIBTCILPEAONIKXKI3LD76AAXHUULM4G5F6UAXWOMCAD) | Groth16 / BLS12-381 on-chain verifier (`pairing_check`) |
| **module-identity-zk** | [`CBGEPGZ2…474VLMO34`](https://stellar.expert/explorer/testnet/contract/CBGEPGZ2JSPK6ZJQCNUBOED3OKFCIOJME356W3TJ6NDFNZ2474VLMO34) | ZK identity provider — proves country ∈ allowed, hidden; `country_of → none` |
| **module-zk-eligibility** | [`CCHSQTUZ…GA3IR3BB`](https://stellar.expert/explorer/testnet/contract/CCHSQTUZNMH6GQ7AH24BQ7VKQE5EAUJORI3VLF3Z34PVIV56GA3IR3BB) | compliance module gating on the ZK `is_verified` flag — a boolean, no country read |
| **compliance (ZK token)** | [`CDBDUQ4S…K3PY377`](https://stellar.expert/explorer/testnet/contract/CDBDUQ4SKTEPOGEVFV5YZJOQTE4KQQW55KL6ZI7ZXCWNYULRUK3PY377) | second dispatcher instance wired to the ZK-eligibility module |
| **zk-token** | [`CCWYNJQJ…2XTNS`](https://stellar.expert/explorer/testnet/contract/CCWYNJQJCPGJRFAMDNNCH2OCIXOYBNFZM5QKDRODUAHOD3KEIPE2XTNS) | RWA token whose transfers gate on ZK eligibility (recipient privacy) |

**Demo accounts:**

| Account | Address | Role |
|---|---|---|
| **deployer / admin** | [`GBV24FM5…WAHJ3ZJJZ`](https://stellar.expert/explorer/testnet/account/GBV24FM5FP6Q736N2JS57N3EGUTGVOLBVP34PXKKGMCDDKEWAHJ3ZJJZ) | issuer / attestor (mints, sets identities) |
| **alice** | [`GABGIKC7…2HNNQRZY`](https://stellar.expert/explorer/testnet/account/GABGIKC77WTE3TSCCA4GLQOLAUOQGJESNUFCGCHLI4HFWNCS2HNNQRZY) | US — compliant holder |
| **bob** | [`GB3IWVIG…XE2TTVJ5`](https://stellar.expert/explorer/testnet/account/GB3IWVIGHCDSQGA7ZMGAOENBQYM4MYI4TFVL2CPWLPEWDXNWXE2TTVJ5) | DE — allowed recipient (transfer passes) |
| **carol** | [`GBGFB7RK…Y7SNYDKK`](https://stellar.expert/explorer/testnet/account/GBGFB7RKGK6LP5KKXP6ZRHK7P2U6EDRGTCK3LAM5USCUGWL2Y7SNYDKK) | TR — disallowed (cleartext denial reveals TR; ZK denial does not) |
| **dave** | [`GDUCS55S…XAIFCPA35`](https://stellar.expert/explorer/testnet/account/GDUCS55SNRZNQIDD3TS2WD2BGVV5V56JF7TTQREP6EBFBWBXAIFCPA35) | ZK-eligible recipient (proven via on-chain Groth16; country stays private) |

> Addresses come from the latest `scripts/deploy-testnet.sh` run and are mirrored in [`scripts/deployed.testnet.json`](scripts/deployed.testnet.json). Re-running the deploy script generates a fresh set.

---

## 9. Quickstart

```bash
# 1) build + test the contracts
stellar contract build
cargo test

# 2) (optional) regenerate the ZK proof artifacts
cd zk && npm install && bash build.sh && cd ..

# 3) deploy the full stack to testnet (cleartext + ZK) with a live pass/revert
bash scripts/deploy-testnet.sh
cp scripts/deployed.testnet.json web/src/deployed.testnet.json

# 4) run the web demo (needs the Freighter extension on Testnet)
cd web && npm install && npm run dev
```

Prereqs: `stellar` CLI, Rust (≥ 1.91, for soroban-sdk 26), `wasm32v1-none` target, Node, and (for ZK regeneration) `circom` + `snarkjs`.

---

## 10. Honest caveats

**Demonstration-grade — not production:**
- Contracts are **unaudited**; the dispatcher/token are a self-contained reference (align with OZ's module ABI is future work).
- ZK is demo-grade. (BLS12-381 itself is the right, *required* curve — Soroban's on-chain crypto only supports it, and it's the stronger one at 128-bit. The caveat is the **hash, not the curve**: circomlib's Poseidon ships **BN254-tuned constants**, and we run it over BLS12-381, so it's a non-standard hash parameterization — the production fix is BLS12-381-proper Poseidon constants, not a different curve.) Also: the trusted setup is a **single local contribution**, the demo submits a **pre-generated** proof, and binding a real **issuer-signed KYC credential** into the circuit is future work (we use a Poseidon commitment).
- The dev `/api/*` bootstrap uses the local CLI admin key (testnet, never shipped).

(Note: the *eligible / not-eligible* boolean being public is **by design**, not a limitation — see §6.4. ZK hides the country, not the eligibility verdict.)

## License

Apache-2.0 (to be aligned with OpenZeppelin `stellar-contracts`).
