# Constella

**Open-source, modular compliance infrastructure for Stellar RWA tokens (Soroban) — with a zero-knowledge privacy layer.**

[![CI](https://github.com/berkay1532/constella/actions/workflows/ci.yml/badge.svg)](https://github.com/berkay1532/constella/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/badge/coverage-89%25%20lines-green.svg)](https://github.com/berkay1532/constella/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

Constella is a library of audit-ready, reusable **compliance modules** plus a **standard module interface** that plug into an OpenZeppelin/ERC-3643-style compliance engine. Issuers compose ready-made rules (holder caps, lock-ups, concentration limits, country restrictions, …) instead of hand-writing them — and can upgrade the identity check to a **zero-knowledge** one that proves an investor is eligible *without revealing their country*.

> Modules = stars forming a compliance constellation. ✨

- **Status:** MVP built & verified — `cargo test` green, all contracts build to wasm, the full stack runs live on Stellar testnet (real pass/revert), and a React demo signs real transfers with Freighter. Phase 2 (ZK) is implemented and wired into the demo.
- **Repo:** `github.com/berkay1532/constella`
- **Docs:** [Quickstart](docs/quickstart.md) · [Testnet evidence](docs/evidence-testnet.md) · [PRD](docs/PRD-Constella.md) · [Architecture](docs/architecture.md) · [Decisions log](docs/DECISIONS.md) · [ZK notes](zk/README.md) · [Contributing](CONTRIBUTING.md) · [License](LICENSE)

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
  module-lockup/                │
  module-max-balance/           │ the self-contained / cleartext compliance modules
  module-country-restrict/      │ (CountryRestrict + MaxInvestorsPerCountry read
  module-denylist/              │  the identity layer)
  module-max-investors-per-country/ │
  module-transfer-window/       ┘
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

### 4.3 The module library

| Module | Rule | Type | How |
|---|---|---|---|
| **MaxHolders** | ≤ N distinct holders | stateful | keeps its **own balance mirror** + holder count from the post-event stream |
| **Lockup** | no transfer for T seconds after acquiring | stateful (time) | records acquisition ledger time per holder; gate = `now ≥ acquired + T` |
| **MaxBalance** | ≤ cap per holder | stateful mirror | own balance mirror; gate = `balance(to) + amount ≤ cap` |
| **CountryRestrict** | recipient's country ∈ allowed | identity-dependent | reads `country_of(to)` from the **identity layer** |
| **Denylist** | sanctioned addresses may not send/receive | stateful (own set) | admin-managed blocklist; deny if `from` or `to` is listed |
| **MaxInvestorsPerCountry** | ≤ N distinct holders per country | identity-dependent + stateful mirror | buckets holders by `country_of`; own mirror tracks per-country holder counts via holder transitions |
| **TransferWindow** | freeze + time-window on all movement | config only | admin `pause`/`unpause` + optional `[open_from, open_until]` ledger-time window |

> **Soroban re-entrancy:** a module **cannot call back into the token** mid-transfer (the host forbids re-entering a contract already on the call stack). So balance-dependent modules (MaxHolders, MaxBalance, MaxInvestorsPerCountry) **never read the token** — they maintain their own balance mirror from `created`/`transferred`/`destroyed`. This is why they must be registered on the post-event hooks from genesis.

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

**In-browser proving (client-side).** The eligibility proof is generated **in the holder's browser** — the private country never leaves the device and no server or admin is involved. `web/src/zk/prove.ts` runs `snarkjs.groth16.fullProve` against the served circuit `.wasm`/`.zkey`; `web/src/zk/encode.ts` is a TypeScript port of `tools/zk-encode` (byte-for-byte verified against the Rust output) that formats the proof for the contract; the holder then signs `register_self` + `prove_eligibility` with their own wallet. Verified end-to-end on testnet — a fresh browser-generated proof flips `is_verified` to `true` on-chain (`register_self` tx [`fceb3b23…`](https://stellar.expert/explorer/testnet/tx/fceb3b23461f50d866a3da48cefc3d5941465869199af31a1c1915c84965ba44), `prove_eligibility` tx [`8d2f42b8…`](https://stellar.expert/explorer/testnet/tx/8d2f42b8bafcbb9c380bece07d1e3ea0583dec6b5cd947c09d534f2d1459ed32)). Production hardening (trusted-setup ceremony, real KYC-attested commitments) is the focus of the next phase.

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

Network: **testnet** · RPC `https://soroban-testnet.stellar.org` · passphrase `Test SDF Network ; September 2015`. Every contract below is live and explorable — click an address. Addresses are the latest `scripts/deploy-testnet.sh` run, mirrored in [`scripts/deployed.testnet.json`](scripts/deployed.testnet.json).

**Core compliance stack (cleartext path):**

| Contract | Address | What it does |
|---|---|---|
| **demo-token** | [`CCYCMBQH…ZD4GAB`](https://stellar.expert/explorer/testnet/contract/CCYCMBQHIBE2VQRPZ437MX32OCQIZHK5RKW4G3VTFCUQFLRXQ7ZD4GAB) | SEP-41-style permissioned RWA token; routes every mint/transfer through the dispatcher |
| **compliance** | [`CDUYCEJU…Y4VZO4`](https://stellar.expert/explorer/testnet/contract/CDUYCEJUVZY6GOLLCL3C5IWL6RE3HR3CX2KDP7BDEBSUGAAXVYY4VZO4) | the dispatcher (engine): per-hook module registry, AND-combined pre-checks, fan-out post-events |
| **identity-mock** | [`CAU6PCZP…DFTASF`](https://stellar.expert/explorer/testnet/contract/CAU6PCZPMPG53X2CI62NBWBHIEU5YC5ILQ5VIR5KYD7ZVLWLR4DFTASF) | cleartext attestor — `country_of` / `is_verified` (Layer 1 identity) |
| **module-country-restrict** | [`CCZIAQWQ…KBVRME`](https://stellar.expert/explorer/testnet/contract/CCZIAQWQ25DBQJPJOTQXZMXAKXKWQOT5SZFYUDVHNC6RAHO3D3KBVRME) | recipient's country ∈ allowed (US, DE); reads the identity layer |
| **module-max-holders** | [`CBATUCBJ…QEZA23`](https://stellar.expert/explorer/testnet/contract/CBATUCBJAEAYIEMRYFNLIBOQVBC6I2DAJHXMYFJYPIQONV2SOZQEZA23) | ≤ 5 distinct holders; self-tracks a balance mirror from the event stream |
| **module-max-balance** | [`CAU7VS7O…37FMWT`](https://stellar.expert/explorer/testnet/contract/CAU7VS7OU6ZSNKDA4SXCT5X6NOZHN3PSJEQDMPXE7QXNT3VFDS37FMWT) | ≤ cap per holder; self-tracks a balance mirror |
| **module-lockup** | [`CCYALEX7…CZRE2O`](https://stellar.expert/explorer/testnet/contract/CCYALEX725WODLPA7KBCDUFSGEY256THIXHSWBLO5C4XT7QWALCZRE2O) | no transfer for T seconds after acquiring (T = 0 in the demo) |

**Reference compliant token (W2/W3 modules — a second self-contained stack):**

| Contract | Address | What it does |
|---|---|---|
| **reference token** | [`CCG5GNJT…3PZO4B`](https://stellar.expert/explorer/testnet/contract/CCG5GNJTSAMCZGR7ZQGSBRQCHPW6AZ2OQBRGCJWXJPIVEUTEMU3PZO4B) | permissioned token wired to the three new modules; live pass/revert demonstrated on deploy |
| **compliance** | [`CAB6OWKH…6NKUYW`](https://stellar.expert/explorer/testnet/contract/CAB6OWKHEBYH25ZPC4FRYELQD3GHHY7Z3S7SMQRE6DJDXVISBW6NKUYW) | second dispatcher instance for the reference token |
| **module-denylist** | [`CCSN74QS…STREYB`](https://stellar.expert/explorer/testnet/contract/CCSN74QSZG5GTQBPEVQI2A67BSYQJ5V7A6UBU2D24PS4BFUL3MSTREYB) | sanctions blocklist — deny if `from`/`to` is listed |
| **module-max-investors-per-country** | [`CCQTLEBE…TYVOOB`](https://stellar.expert/explorer/testnet/contract/CCQTLEBEG2O2EZA5GGTVBF4FFOXIQQHTGRJLKAK54QY3D4A67ITYVOOB) | ≤ 1 holder/country in the demo; a 2nd US holder reverts |
| **module-transfer-window** | [`CARJAMBT…EDG6KO`](https://stellar.expert/explorer/testnet/contract/CARJAMBTDIQXQHDVNYDUTXGYKSHSF773W66VZY7XS4PY5GBNAPEDG6KO) | freeze + time-window; `pause()` makes mint/transfer revert |

**Zero-knowledge layer (Phase 2 — proof generated in-browser):**

| Contract | Address | What it does |
|---|---|---|
| **zk-verifier** | [`CC3E26EG…6H66XE`](https://stellar.expert/explorer/testnet/contract/CC3E26EGAYYA7A2K3RHXEYTIX4TCL7MTFSYTKQUKNKU3TZHEYS6H66XE) | Groth16 / BLS12-381 on-chain verifier (`pairing_check`) |
| **module-identity-zk** | [`CDZDNLUK…UMWPOY`](https://stellar.expert/explorer/testnet/contract/CDZDNLUKIT3HOAO7ZL6EH7R4G5Q6QTUYM3DPK5QT6FM5GAPRBYUMWPOY) | ZK identity provider — `register_self` + `prove_eligibility`; proves country ∈ allowed, hidden |
| **module-zk-eligibility** | [`CD73U4NS…CNU2WM`](https://stellar.expert/explorer/testnet/contract/CD73U4NSIA3DVVODWUSR6WVBITNZTYAFZQIKD7WYGV4WYBSMRFCNU2WM) | compliance module gating on the ZK `is_verified` flag |
| **compliance (ZK token)** | [`CCXLNCQV…XTIBXO`](https://stellar.expert/explorer/testnet/contract/CCXLNCQVXZ7EREVMJSO3GAHTQ7ZQOPR2B43YLODBXA7OP7R3MLXTIBXO) | dispatcher instance wired to the ZK-eligibility module |
| **zk-token** | [`CB5673SC…EQWBVE`](https://stellar.expert/explorer/testnet/contract/CB5673SCMIDVRAO7JA7367HIJO4WW5GM247QBKQJNATKKHVQL6EQWBVE) | RWA token whose transfers gate on ZK eligibility (recipient privacy) |

**Demo accounts:**

| Account | Address | Role |
|---|---|---|
| **deployer / admin** | [`GDXK5YGK…W3VIFC`](https://stellar.expert/explorer/testnet/account/GDXK5YGKCYYQYIEGWQNVTQXN7MK6VDDCA5UV4ZYP7TWWEGTMVSW3VIFC) | issuer / attestor |
| **alice** | [`GCDA5EJ4…C2U4LO`](https://stellar.expert/explorer/testnet/account/GCDA5EJ4HR5IOQ2GFGYEIE5FC7GUOA7XYSFS552TRSXHLIVNLAC2U4LO) | US — compliant holder |
| **bob** | [`GBXBMWAK…6LJBJK`](https://stellar.expert/explorer/testnet/account/GBXBMWAK5UHXXV7QLFKPGBO2ZJQMFHYWPBFX7GW3NMHDYCU6NY6LJBJK) | DE — allowed recipient |
| **carol** | [`GCABGRS5…FB5BZW`](https://stellar.expert/explorer/testnet/account/GCABGRS5ZDW2FSZQIGULCUTUW5W74RETDJZOVG6IJ4YHDCTLOTFB5BZW) | TR — disallowed |
| **frank** | [`GAUKLVUW…AXEUZG`](https://stellar.expert/explorer/testnet/account/GAUKLVUWT6OILH3IC62Z7ELEFU7V7V7OTRRGSKLJWN77EAI6DSAXEUZG) | US — trips the per-country investor cap |
| **dave** | [`GCZB5ZFV…4NUAUH`](https://stellar.expert/explorer/testnet/account/GCZB5ZFVGOBFWIYFLTASOWFLDMB7U2JUV6XPN36FRTQGPPCN3X4NUAUH) | ZK-eligible recipient (proven via on-chain Groth16) |

> Re-running the deploy script generates a fresh set of addresses. For a curated list of live pass/revert **transaction hashes** proving each new module on-chain, see [`docs/evidence-testnet.md`](docs/evidence-testnet.md) (regenerate with `bash scripts/capture-evidence.sh`).

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

> **Building your own compliant token?** [`docs/quickstart.md`](docs/quickstart.md) is a step-by-step guide to composing the module library into a token of your own — pick your rules, deploy, and prove they bite — with both a guided (script) and a manual path.

---

## 10. Honest caveats

**Demonstration-grade — not production:**
- Contracts are **unaudited**; the dispatcher/token are a self-contained reference (align with OZ's module ABI is future work).
- ZK is demo-grade. (BLS12-381 itself is the right, *required* curve — Soroban's on-chain crypto only supports it, and it's the stronger one at 128-bit. The caveat is the **hash, not the curve**: circomlib's Poseidon ships **BN254-tuned constants**, and we run it over BLS12-381, so it's a non-standard hash parameterization — the production fix is BLS12-381-proper Poseidon constants, not a different curve.) Also: the trusted setup is a **single local contribution**, the demo submits a **pre-generated** proof, and binding a real **issuer-signed KYC credential** into the circuit is future work (we use a Poseidon commitment).
- The dev `/api/*` bootstrap uses the local CLI admin key (testnet, never shipped).

(Note: the *eligible / not-eligible* boolean being public is **by design**, not a limitation — see §6.4. ZK hides the country, not the eligibility verdict.)

## License

Apache-2.0 (to be aligned with OpenZeppelin `stellar-contracts`).
