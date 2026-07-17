# Constellation Launch Wizard + Token Console (SP2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A no-code web experience where anyone connects Freighter and, in **one signed transaction**, launches their own real compliance token on Stellar testnet (choosing/configuring the 7 modules), then exercises it from a console to see restrictions enforce live.

**Architecture:** One-time platform bootstrap deploys the shared `Hub` + 7 modules to testnet and records their IDs in `web/src/hub.testnet.json`. The frontend (React 18 + Vite + react-router) calls `hub.launch(LaunchConfig)` client-side via Freighter — no backend, no contract changes. A `hub.ts` layer encodes `LaunchConfig` as an `ScVal` (the existing manual `ScMap` pattern) and wraps the hub's forwarders/reads.

**Tech Stack:** React 18, TypeScript (strict), Vite 6, `@stellar/stellar-sdk` v15, `@stellar/freighter-api`, `react-router-dom` (new), plain CSS.

## Global Constraints

- English only (code, comments, UI copy, commits). Everything committed to the repo is English.
- No Soroban contract changes. The hub is complete (7/7); the token stays the generic `demo-token` (constructor `(admin, hub_address)`, no name/symbol).
- No backend/server on the product path — all launch/console mutations are Freighter-signed client-side. The existing dev-only Vite middleware stays for the legacy `/zk` demo only.
- Network: testnet, from `web/src/hub.testnet.json` (`rpcUrl https://soroban-testnet.stellar.org`, passphrase `Test SDF Network ; September 2015`). Real launches — no mocks.
- Never put a secret in the browser bundle. The platform-admin key signs only the one-time bootstrap script.
- Do NOT break the existing ZK eligibility + transfer demo — it moves to `/zk` unchanged in behavior.
- Reuse the proven `stellar.ts` shape: build → `server.prepareTransaction` (compliance rejections surface here, pre-signature) → Freighter-sign → `sendTransaction` → poll `getTransaction`.
- `LaunchConfig` `#[contracttype]` struct encodes as a symbol-keyed `ScMap` **sorted by key**: `admin, country_restrict, denylist, lockup, max_balance, max_holders, max_investors, transfer_window`.
- `tsc --noEmit` and `vite build` must stay clean after every task.

## File Structure

- Create `scripts/bootstrap-hub-testnet.sh` — one-time platform bootstrap; emits `web/src/hub.testnet.json`.
- Create `web/src/hub.testnet.json` — committed platform config (hub + 7 module IDs).
- Modify `web/src/stellar.ts` — export shared primitives (`server`, `NP`, `buildFrom`, `signSendPoll`, `addr`, `i128`, `SignFn`, `SendResult`) for reuse; no behavior change.
- Create `web/src/hub.ts` — `LaunchConfig` type, `launchConfigScVal`, `launchToken`, and console wrappers (mint/attest/forwarders/reads).
- Create `web/scripts/verify-launch-encoder.mjs` — golden round-trip + sorted-key test for `launchConfigScVal`.
- Create `web/src/tokenStore.ts` — `localStorage` persistence of launched tokens per admin.
- Create `web/src/wallet.tsx` — `WalletProvider` + `useWallet` (connect/address/sign session).
- Modify `web/src/main.tsx` — wrap in `<BrowserRouter>` + `<WalletProvider>`.
- Modify `web/src/App.tsx` — becomes the router shell (nav + `<Routes>`).
- Create `web/src/routes/Landing.tsx`, `web/src/routes/LaunchWizard.tsx`, `web/src/routes/TokenConsole.tsx`, `web/src/routes/LegacyDemo.tsx` (the current App body, moved verbatim).
- Modify `web/package.json` — add `react-router-dom` + a `verify:launch` script.
- Modify `web/src/styles.css` — small set of Constellation tokens + wizard/console layout classes (functional, not final polish).

---

### Task 1: Platform bootstrap → `hub.testnet.json`

**Files:** Create `scripts/bootstrap-hub-testnet.sh`, `web/src/hub.testnet.json`

**Interfaces:**
- Produces: `web/src/hub.testnet.json` shaped `{ network, rpcUrl, networkPassphrase, hub, modules: { denylist, max_balance, country_restrict, max_holders, lockup, transfer_window, max_investors } }` — consumed by `hub.ts` (Task 2).

- [ ] **Step 1: Write the bootstrap script** — `scripts/bootstrap-hub-testnet.sh`

Follows the exact CLI pattern already used in the hub testnet spikes. Deploys the hub + all 7 shared modules once, uploads token + identity wasm, wires wasm hashes + module addresses, then writes the JSON.

```bash
#!/usr/bin/env bash
# One-time platform bootstrap: deploy the shared multi-tenant Hub + all 7 modules to testnet,
# wire the token/identity wasm + module addresses, and emit web/src/hub.testnet.json.
# The platform-admin (deployer) key signs ONLY this script — never anything in the browser.
set -euo pipefail
cd "$(dirname "$0")/.."
NET=testnet
W=target/wasm32v1-none/release
OUT=web/src/hub.testnet.json

echo "▸ Building wasm…"; cargo build --workspace --release --target wasm32v1-none 2>&1 | tail -1
DEP=$(stellar keys address deployer)

echo "▸ Uploading token + identity wasm…"
TOKHASH=$(stellar contract upload --wasm "$W/constella_demo_token.wasm" --source deployer --network "$NET" 2>/dev/null | tail -1)
IDHASH=$(stellar contract upload --wasm "$W/constella_identity_mock.wasm" --source deployer --network "$NET" 2>/dev/null | tail -1)

echo "▸ Deploying hub…"
HUB=$(stellar contract deploy --wasm "$W/constella_hub.wasm" --source deployer --network "$NET" -- --platform_admin "$DEP" 2>/dev/null | tail -1)

declare -A KIND_WASM=(
  [denylist]=constella_hub_module_denylist
  [max_balance]=constella_hub_module_max_balance
  [country_restrict]=constella_hub_module_country_restrict
  [max_holders]=constella_hub_module_max_holders
  [lockup]=constella_hub_module_lockup
  [transfer_window]=constella_hub_module_transfer_window
  [max_investors]=constella_hub_module_max_investors_per_country
)
declare -A ADDR
for kind in "${!KIND_WASM[@]}"; do
  echo "▸ Deploying module $kind…"
  ADDR[$kind]=$(stellar contract deploy --wasm "$W/${KIND_WASM[$kind]}.wasm" --source deployer --network "$NET" -- --hub "$HUB" 2>/dev/null | tail -1)
done

echo "▸ Platform config on hub…"
stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_token_wasm --hash "$TOKHASH" >/dev/null
stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_identity_wasm --hash "$IDHASH" >/dev/null
for kind in "${!ADDR[@]}"; do
  stellar contract invoke --id "$HUB" --source deployer --network "$NET" -- set_module_addr --kind "$kind" --addr "${ADDR[$kind]}" >/dev/null
done

echo "▸ Writing $OUT…"
cat > "$OUT" <<JSON
{
  "network": "testnet",
  "rpcUrl": "https://soroban-testnet.stellar.org",
  "networkPassphrase": "Test SDF Network ; September 2015",
  "hub": "$HUB",
  "modules": {
    "denylist": "${ADDR[denylist]}",
    "max_balance": "${ADDR[max_balance]}",
    "country_restrict": "${ADDR[country_restrict]}",
    "max_holders": "${ADDR[max_holders]}",
    "lockup": "${ADDR[lockup]}",
    "transfer_window": "${ADDR[transfer_window]}",
    "max_investors": "${ADDR[max_investors]}"
  }
}
JSON
echo "=== BOOTSTRAP DONE ==="; cat "$OUT"
```

- [ ] **Step 2: Run it against testnet**

Run: `bash scripts/bootstrap-hub-testnet.sh`
Expected: `=== BOOTSTRAP DONE ===` followed by a JSON blob with one `hub` C-address and seven module C-addresses (all `C…`, 56 chars). If a `stellar keys address deployer` error appears, the environment lacks a funded `deployer` identity — stop and report (do not invent addresses).

- [ ] **Step 3: Sanity-check + commit**

Run: `test -s web/src/hub.testnet.json && grep -c '"C' web/src/hub.testnet.json` (expect the file non-empty; 8 C-addresses).
```bash
git add scripts/bootstrap-hub-testnet.sh web/src/hub.testnet.json
git commit -m "feat(sp2): platform bootstrap script + committed hub.testnet.json (shared stack on testnet)"
```

---

### Task 2: `hub.ts` launch layer + `stellar.ts` primitive exports + golden test

**Files:** Modify `web/src/stellar.ts`; Create `web/src/hub.ts`, `web/scripts/verify-launch-encoder.mjs`; Modify `web/package.json`

**Interfaces:**
- Consumes: `web/src/hub.testnet.json` (Task 1); `stellar.ts` exports.
- Produces: `LaunchConfig` type; `launchToken(cfg, sign) -> Promise<{ token: string; hash: string }>`; `blankConfig()`; the shared `SignFn` type. Consumed by the Wizard (Task 4) and Console (Task 5).

- [ ] **Step 1: Export shared primitives from `stellar.ts`**

At the point each is defined, add `export` (no behavior change): `server`, `NP`, `buildFrom`, `signSendPoll`, `addr`, `i128`. Also export the `SignFn` type. Concretely, change these existing lines:
- `const server = new rpc.Server(deployed.rpcUrl);` → `export const server = new rpc.Server(deployed.rpcUrl);`
- `const NP = deployed.networkPassphrase;` → `export const NP = deployed.networkPassphrase;`
- `const addr = (a: string) => ...` → `export const addr = (a: string) => ...`
- `const i128 = (n: number | string) => ...` → `export const i128 = (n: number | string) => ...`
- `function buildFrom(...)` → `export function buildFrom(...)`
- `async function signSendPoll(...)` → `export async function signSendPoll(...)`
- `type SignFn = (xdr: string) => Promise<string>;` → `export type SignFn = (xdr: string) => Promise<string>;`
- Ensure `xdr`, `nativeToScVal`, `scValToNative` remain imported (they already are).

- [ ] **Step 2: Write the failing golden test** — `web/scripts/verify-launch-encoder.mjs`

Standalone Node script (mirrors `verify-encoder.mjs`: inline-reconstructs the encoder so no TS loader is needed), asserting (a) the encoded `ScVal` round-trips back to the input config, and (b) the `ScMap` keys are in sorted order.

```js
// Verify launchConfigScVal: round-trips to the input config and emits sorted ScMap keys.
// Inline copy of the encoder (kept in sync with web/src/hub.ts). Run: node web/scripts/verify-launch-encoder.mjs
import { xdr, nativeToScVal, scValToNative } from '@stellar/stellar-sdk';

const addr = (a) => nativeToScVal(a, { type: 'address' });
const i128 = (n) => nativeToScVal(n, { type: 'i128' });
const u32 = (n) => nativeToScVal(n, { type: 'u32' });
const u64 = (n) => nativeToScVal(n, { type: 'u64' });
const u32vec = (arr) => xdr.ScVal.scvVec(arr.map(u32));

function launchConfigScVal(cfg) {
  const e = (k, v) => new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol(k), val: v });
  return xdr.ScVal.scvMap([
    e('admin', addr(cfg.admin)),
    e('country_restrict', u32vec(cfg.country_restrict)),
    e('denylist', xdr.ScVal.scvBool(cfg.denylist)),
    e('lockup', u64(cfg.lockup)),
    e('max_balance', i128(cfg.max_balance)),
    e('max_holders', u32(cfg.max_holders)),
    e('max_investors', u32(cfg.max_investors)),
    e('transfer_window', xdr.ScVal.scvBool(cfg.transfer_window)),
  ]);
}

const cfg = {
  admin: 'GDXK5YGKCYYQYIEGWQNVTQXN7MK6VDDCA5UV4ZYP7TWWEGTMVSW3VIFC',
  denylist: true, max_balance: '1000', country_restrict: [840, 276],
  max_holders: 5, lockup: '3600', transfer_window: false, max_investors: 2,
};
const sv = launchConfigScVal(cfg);
const keys = sv.map().map((en) => en.key().sym().toString());
const sorted = [...keys].sort();
if (JSON.stringify(keys) !== JSON.stringify(sorted)) {
  console.error('MISMATCH: ScMap keys not sorted\n  got:', keys, '\n  want:', sorted);
  process.exit(1);
}
const back = scValToNative(sv);
const norm = (o) => ({ ...o, max_balance: String(o.max_balance), lockup: String(o.lockup),
  country_restrict: o.country_restrict.map(Number) });
if (JSON.stringify(norm(back)) !== JSON.stringify(norm(cfg))) {
  console.error('MISMATCH: round-trip\n  got:', norm(back), '\n  want:', norm(cfg));
  process.exit(1);
}
console.log('✅ launchConfigScVal: keys sorted + round-trips to input config');
```

- [ ] **Step 3: Run it to verify it fails**

Run: `node web/scripts/verify-launch-encoder.mjs`
Expected: FAIL — `Cannot find module` / no encoder yet is fine as the RED signal, OR (since the script is self-contained) it may already pass. If it passes here, that only proves the inline copy is self-consistent; Step 4 makes `hub.ts` the real source and Step 6 re-runs. Treat a non-zero exit as RED.

- [ ] **Step 4: Implement `hub.ts`** — `web/src/hub.ts`

```ts
import { xdr, nativeToScVal, scValToNative, TransactionBuilder, rpc, Account } from '@stellar/stellar-sdk';
import { server, NP, buildFrom, addr, i128, type SignFn } from './stellar';
import hub from './hub.testnet.json';

export { hub };

export type LaunchConfig = {
  admin: string;
  denylist: boolean;
  max_balance: string; // i128 as decimal string; '0' = off
  country_restrict: number[]; // ISO numeric codes; [] = off
  max_holders: number; // 0 = off
  lockup: number; // seconds; 0 = off
  transfer_window: boolean;
  max_investors: number; // per-country cap; 0 = off
};

export const blankConfig = (admin: string): LaunchConfig => ({
  admin, denylist: false, max_balance: '0', country_restrict: [],
  max_holders: 0, lockup: 0, transfer_window: false, max_investors: 0,
});

const u32 = (n: number) => nativeToScVal(n, { type: 'u32' });
const u64 = (n: number) => nativeToScVal(n, { type: 'u64' });
const u32vec = (arr: number[]) => xdr.ScVal.scvVec(arr.map(u32));

/** LaunchConfig #[contracttype] struct -> symbol-keyed ScMap sorted by key. */
export function launchConfigScVal(cfg: LaunchConfig): xdr.ScVal {
  const e = (k: string, v: xdr.ScVal) => new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol(k), val: v });
  return xdr.ScVal.scvMap([
    e('admin', addr(cfg.admin) as unknown as xdr.ScVal),
    e('country_restrict', u32vec(cfg.country_restrict)),
    e('denylist', xdr.ScVal.scvBool(cfg.denylist)),
    e('lockup', u64(cfg.lockup)),
    e('max_balance', i128(cfg.max_balance) as unknown as xdr.ScVal),
    e('max_holders', u32(cfg.max_holders)),
    e('max_investors', u32(cfg.max_investors)),
    e('transfer_window', xdr.ScVal.scvBool(cfg.transfer_window)),
  ]);
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** One-signature launch: hub.launch(config) -> the deployed token address. */
export async function launchToken(cfg: LaunchConfig, sign: SignFn): Promise<{ token: string; hash: string }> {
  const acc = await server.getAccount(cfg.admin);
  const tx = buildFrom(acc, hub.hub, 'launch', [launchConfigScVal(cfg) as unknown as ReturnType<typeof addr>]);
  const prepared = await server.prepareTransaction(tx);
  const signedXDR = await sign(prepared.toXDR());
  const sent = await server.sendTransaction(TransactionBuilder.fromXDR(signedXDR, NP));
  if (sent.status === 'ERROR') throw new Error('launch submit error');
  let got = await server.getTransaction(sent.hash);
  for (let i = 0; i < 30 && got.status === rpc.Api.GetTransactionStatus.NOT_FOUND; i++) {
    await sleep(1000);
    got = await server.getTransaction(sent.hash);
  }
  if (got.status !== rpc.Api.GetTransactionStatus.SUCCESS) throw new Error(`launch did not succeed: ${got.status}`);
  const result = scValToNative(got.returnValue!) as { token: string };
  return { token: result.token, hash: sent.hash };
}
```
Note on the `as unknown as` casts: `addr`/`i128` return the SDK's `nativeToScVal` result type while `ScMapEntry`/`c.call` are typed against `xdr.ScVal`; they are the same runtime object. Keep the casts narrow and localized (this mirrors the existing `scvBytes` cast note in `stellar.ts`).

- [ ] **Step 5: Add `react-router-dom` + verify script to `web/package.json`**

Add to `dependencies`: `"react-router-dom": "^6.28.0"`. Add to `scripts`: `"verify:launch": "node web/scripts/verify-launch-encoder.mjs"`. Then:
Run: `cd web && npm install` (installs react-router-dom; needed here so later tasks compile).

- [ ] **Step 6: Run golden + typecheck + build, then commit**

```bash
node web/scripts/verify-launch-encoder.mjs   # expect ✅
cd web && npx tsc --noEmit && npm run build   # both clean
```
Then:
```bash
git add web/src/stellar.ts web/src/hub.ts web/scripts/verify-launch-encoder.mjs web/package.json web/package-lock.json
git commit -m "feat(sp2): hub.ts launch layer (LaunchConfig ScVal encoder + launchToken) + golden test"
```

---

### Task 3: Routing + WalletContext + move legacy demo to `/zk`

**Files:** Create `web/src/wallet.tsx`, `web/src/routes/LegacyDemo.tsx`, `web/src/routes/Landing.tsx`, `web/src/routes/LaunchWizard.tsx` (stub), `web/src/routes/TokenConsole.tsx` (stub); Modify `web/src/main.tsx`, `web/src/App.tsx`, `web/src/styles.css`

**Interfaces:**
- Produces: `useWallet() -> { address: string | null; connect(): Promise<void>; sign: SignFn; busy: boolean; error: string }` (consumed by all routes). Routes: `/` Landing, `/launch` Wizard, `/token/:id` Console, `/zk` LegacyDemo.

- [ ] **Step 1: WalletProvider** — `web/src/wallet.tsx`

Lifts the wallet session out of the old `App`. `sign` is `(xdr) => signXDR(xdr, address)` — the exact callback shape the existing `submit*` functions expect.
```tsx
import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { connectWallet, currentAddress, signXDR } from './freighter';
import type { SignFn } from './stellar';

type WalletCtx = { address: string | null; connect: () => Promise<void>; sign: SignFn; busy: boolean; error: string };
const Ctx = createContext<WalletCtx | null>(null);

export function WalletProvider({ children }: { children: ReactNode }) {
  const [address, setAddress] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => { currentAddress().then((a) => a && setAddress(a)); }, []);
  const connect = async () => {
    setBusy(true); setError('');
    try { setAddress(await connectWallet()); }
    catch (e) { setError(String((e as Error).message || e)); }
    finally { setBusy(false); }
  };
  const sign: SignFn = (xdr) => signXDR(xdr, address!);
  return <Ctx.Provider value={{ address, connect, sign, busy, error }}>{children}</Ctx.Provider>;
}
export function useWallet() {
  const c = useContext(Ctx);
  if (!c) throw new Error('useWallet outside WalletProvider');
  return c;
}
```

- [ ] **Step 2: Move the current App body into `LegacyDemo`** — `web/src/routes/LegacyDemo.tsx`

Copy the ENTIRE current `web/src/App.tsx` into `web/src/routes/LegacyDemo.tsx`, renaming `export function App()` to `export function LegacyDemo()`. Fix the relative imports (they move one directory deeper): `./stellar` → `../stellar`, `./freighter` → `../freighter`, `./zk/...` → `../zk/...`. Do NOT change any logic — this preserves the ZK + transfer demo verbatim. (It keeps its own local wallet `useState`; it does not need to consume `useWallet`.)

- [ ] **Step 3: App shell + stub routes** — `web/src/App.tsx`

Replace the file contents with a router shell:
```tsx
import { Link, Routes, Route } from 'react-router-dom';
import { Landing } from './routes/Landing';
import { LaunchWizard } from './routes/LaunchWizard';
import { TokenConsole } from './routes/TokenConsole';
import { LegacyDemo } from './routes/LegacyDemo';
import { useWallet } from './wallet';

export function App() {
  const { address, connect, busy } = useWallet();
  return (
    <div className="wrap">
      <nav className="topnav">
        <Link to="/" className="brand">✨ Constella</Link>
        <div className="navlinks">
          <Link to="/launch">Launch</Link>
          <Link to="/zk">ZK demo</Link>
          {address
            ? <span className="pill">{address.slice(0, 4)}…{address.slice(-4)}</span>
            : <button className="send" onClick={connect} disabled={busy}>Connect</button>}
        </div>
      </nav>
      <Routes>
        <Route path="/" element={<Landing />} />
        <Route path="/launch" element={<LaunchWizard />} />
        <Route path="/token/:id" element={<TokenConsole />} />
        <Route path="/zk" element={<LegacyDemo />} />
      </Routes>
    </div>
  );
}
```

- [ ] **Step 4: Stub Landing / Wizard / Console**

`web/src/routes/Landing.tsx`:
```tsx
import { Link } from 'react-router-dom';
export function Landing() {
  return (
    <section className="card hero">
      <h1>Launch your own compliance token</h1>
      <p>Pick from seven on-chain compliance modules and deploy a real, restricted token on Stellar testnet — in one signature. No code.</p>
      <Link to="/launch" className="send">Launch a token →</Link>
      <p className="muted">Curious about the privacy tech? See the <Link to="/zk">zero-knowledge eligibility demo</Link>.</p>
    </section>
  );
}
```
`web/src/routes/LaunchWizard.tsx` (stub): `export function LaunchWizard() { return <section className="card"><h2>Launch wizard</h2><p>Coming in Task 4.</p></section>; }`
`web/src/routes/TokenConsole.tsx` (stub): `export function TokenConsole() { return <section className="card"><h2>Token console</h2><p>Coming in Task 5.</p></section>; }`

- [ ] **Step 5: Wire router + provider** — `web/src/main.tsx`

```tsx
import React from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { App } from './App';
import { WalletProvider } from './wallet';
import './styles.css';

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <BrowserRouter>
      <WalletProvider>
        <App />
      </WalletProvider>
    </BrowserRouter>
  </React.StrictMode>,
);
```

- [ ] **Step 6: Minimal nav/hero styles** — append to `web/src/styles.css`

```css
.topnav { display:flex; justify-content:space-between; align-items:center; padding:16px 0; margin-bottom:8px; }
.topnav .brand { font-weight:700; font-size:1.15rem; text-decoration:none; color:var(--accent); }
.navlinks { display:flex; gap:16px; align-items:center; }
.navlinks a { color:var(--fg); text-decoration:none; opacity:.85; }
.navlinks a:hover { opacity:1; }
.hero h1 { font-size:1.8rem; margin:0 0 12px; }
.muted { opacity:.7; font-size:.9rem; }
.wizard-steps { display:flex; gap:8px; margin-bottom:16px; }
.wizard-steps .step { flex:1; text-align:center; padding:8px; border-radius:8px; background:var(--card); opacity:.5; }
.wizard-steps .step.active { opacity:1; border:1px solid var(--accent); }
.field { display:flex; flex-direction:column; gap:4px; margin:10px 0; }
.field label { font-size:.85rem; opacity:.8; }
.mod-row { display:flex; justify-content:space-between; align-items:center; gap:12px; padding:10px 0; border-bottom:1px solid rgba(255,255,255,.06); }
```

- [ ] **Step 7: Typecheck, build, manual smoke, commit**

```bash
cd web && npx tsc --noEmit && npm run build
```
Manual: `npm run dev`, confirm `/` (Landing), `/launch` (stub), `/zk` (the full legacy demo — connect wallet, ZK prove still works) all render and the top nav connects the wallet. Then:
```bash
git add web/src/wallet.tsx web/src/App.tsx web/src/main.tsx web/src/routes web/src/styles.css
git commit -m "feat(sp2): react-router shell + WalletProvider; move legacy demo to /zk unchanged"
```

---

### Task 4: Launch Wizard — 4-step flow → real one-signature launch

**Files:** Modify `web/src/routes/LaunchWizard.tsx`; Create `web/src/tokenStore.ts`

**Interfaces:**
- Consumes: `useWallet()`, `launchToken`, `LaunchConfig`, `blankConfig` (Task 2).
- Produces: `saveToken(rec)` / `listTokens(admin)` / `getToken(id)` in `tokenStore.ts` (consumed by Console, Task 5).

- [ ] **Step 1: tokenStore** — `web/src/tokenStore.ts`

```ts
import type { LaunchConfig } from './hub';
export type TokenRecord = { id: string; admin: string; config: LaunchConfig; hash: string; createdAt: number };
const key = (admin: string) => `constella.tokens.${admin}`;

export function listTokens(admin: string): TokenRecord[] {
  try { return JSON.parse(localStorage.getItem(key(admin)) || '[]'); } catch { return []; }
}
export function saveToken(rec: TokenRecord): void {
  const all = listTokens(rec.admin).filter((t) => t.id !== rec.id);
  all.unshift(rec);
  localStorage.setItem(key(rec.admin), JSON.stringify(all));
}
export function getToken(admin: string, id: string): TokenRecord | undefined {
  return listTokens(admin).find((t) => t.id === id);
}
```

- [ ] **Step 2: Wizard reducer + step 1/2 (Basics + Compliance)** — `web/src/routes/LaunchWizard.tsx`

State via `useReducer` over `LaunchConfig` plus a `step` index. `createdAt` must be passed at save time (`Date.now()`), not stored in module scope.
```tsx
import { useReducer, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useWallet } from '../wallet';
import { launchToken, blankConfig, type LaunchConfig } from '../hub';
import { saveToken } from '../tokenStore';

type Action = { field: keyof LaunchConfig; value: LaunchConfig[keyof LaunchConfig] };
const reducer = (s: LaunchConfig, a: Action): LaunchConfig => ({ ...s, [a.field]: a.value });

const COUNTRIES = [ { code: 840, name: 'United States' }, { code: 276, name: 'Germany' },
  { code: 792, name: 'Turkey' }, { code: 250, name: 'France' } ];

export function LaunchWizard() {
  const { address, connect, sign, busy } = useWallet();
  const [cfg, dispatch] = useReducer(reducer, blankConfig(address || ''));
  const [step, setStep] = useState(0);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const nav = useNavigate();
  const set = (field: keyof LaunchConfig, value: LaunchConfig[keyof LaunchConfig]) => dispatch({ field, value });

  if (!address) {
    return <section className="card"><h2>Launch</h2><p>Connect your wallet to begin — it becomes the token's issuer/admin.</p>
      <button className="send" onClick={connect} disabled={busy}>Connect Freighter</button></section>;
  }
  // keep admin in sync with the connected wallet
  if (cfg.admin !== address) set('admin', address);

  const toggleCountry = (code: number) => set('country_restrict',
    cfg.country_restrict.includes(code) ? cfg.country_restrict.filter((c) => c !== code) : [...cfg.country_restrict, code]);

  const onLaunch = async () => {
    setError(''); setStatus('Preparing…');
    try {
      setStatus('Awaiting signature…');
      const { token, hash } = await launchToken(cfg, sign);
      saveToken({ id: token, admin: address, config: cfg, hash, createdAt: Date.now() });
      setStatus('Launched!');
      nav(`/token/${token}`);
    } catch (e) { setError(String((e as Error).message || e)); setStatus(''); }
  };

  return (
    <section className="card">
      <div className="wizard-steps">
        {['Basics', 'Compliance', 'Review'].map((s, i) =>
          <div key={s} className={`step ${i === step ? 'active' : ''}`}>{i + 1}. {s}</div>)}
      </div>

      {step === 0 && (
        <div>
          <h2>Token basics</h2>
          <p>Issuer / admin: <span className="pill">{address.slice(0,6)}…{address.slice(-4)}</span></p>
          <p className="muted">A generic compliant token is deployed under your control. You configure its restrictions next.</p>
          <button className="send" onClick={() => setStep(1)}>Next →</button>
        </div>
      )}

      {step === 1 && (
        <div>
          <h2>Compliance modules</h2>
          <div className="mod-row"><span>Denylist (block specific accounts)</span>
            <input type="checkbox" checked={cfg.denylist} onChange={(e) => set('denylist', e.target.checked)} /></div>
          <div className="mod-row"><span>Max balance per holder</span>
            <input type="number" min={0} value={cfg.max_balance} onChange={(e) => set('max_balance', e.target.value || '0')} style={{width:120}} /></div>
          <div className="mod-row"><span>Max holders</span>
            <input type="number" min={0} value={cfg.max_holders} onChange={(e) => set('max_holders', Number(e.target.value))} style={{width:120}} /></div>
          <div className="mod-row"><span>Lockup (seconds)</span>
            <input type="number" min={0} value={cfg.lockup} onChange={(e) => set('lockup', Number(e.target.value))} style={{width:120}} /></div>
          <div className="mod-row"><span>Transfer window (start paused/windowed)</span>
            <input type="checkbox" checked={cfg.transfer_window} onChange={(e) => set('transfer_window', e.target.checked)} /></div>
          <div className="mod-row"><span>Max investors per country</span>
            <input type="number" min={0} value={cfg.max_investors} onChange={(e) => set('max_investors', Number(e.target.value))} style={{width:120}} /></div>
          <div className="field"><label>Country allow-list (country restrict)</label>
            <div style={{display:'flex',gap:12,flexWrap:'wrap'}}>
              {COUNTRIES.map((c) => <label key={c.code} style={{display:'flex',gap:4,alignItems:'center'}}>
                <input type="checkbox" checked={cfg.country_restrict.includes(c.code)} onChange={() => toggleCountry(c.code)} />{c.name}</label>)}
            </div>
          </div>
          {cfg.country_restrict.length > 0 && cfg.max_investors > 0 &&
            <p className="muted">Country-restrict and max-investors share one identity for this token.</p>}
          <button className="send" onClick={() => setStep(0)}>← Back</button>{' '}
          <button className="send" onClick={() => setStep(2)}>Review →</button>
        </div>
      )}

      {step === 2 && (
        <div>
          <h2>Review &amp; launch</h2>
          <ul>
            <li>Admin: {address}</li>
            <li>Denylist: {cfg.denylist ? 'on' : 'off'}</li>
            <li>Max balance: {cfg.max_balance === '0' ? 'off' : cfg.max_balance}</li>
            <li>Country allow-list: {cfg.country_restrict.length ? cfg.country_restrict.join(', ') : 'off'}</li>
            <li>Max holders: {cfg.max_holders || 'off'}</li>
            <li>Lockup: {cfg.lockup ? `${cfg.lockup}s` : 'off'}</li>
            <li>Transfer window: {cfg.transfer_window ? 'on' : 'off'}</li>
            <li>Max investors/country: {cfg.max_investors || 'off'}</li>
          </ul>
          <button className="send" onClick={() => setStep(1)}>← Back</button>{' '}
          <button className="send" onClick={onLaunch} disabled={!!status && !error}>Launch (one signature)</button>
          {status && <p className="muted">{status}</p>}
          {error && <div className="result denied">{error}</div>}
        </div>
      )}
    </section>
  );
}
```

- [ ] **Step 3: Typecheck, build, commit**

```bash
cd web && npx tsc --noEmit && npm run build
```
```bash
git add web/src/routes/LaunchWizard.tsx web/src/tokenStore.ts
git commit -m "feat(sp2): launch wizard — 4-step config -> one-signature hub.launch + localStorage"
```

---

### Task 5: Token Console — exercise & verify restrictions live

**Files:** Modify `web/src/hub.ts` (console wrappers), `web/src/routes/TokenConsole.tsx`

**Interfaces:**
- Consumes: `useWallet()`, `getToken` (Task 4), the token's `LaunchConfig`.
- Produces: `hub.ts` console wrappers — mutators `mint`, `attestCountry`, `setInvestorCap`, `setMaxBalance`, `setMaxHolders`, `pauseToken`, `unpauseToken`, `addToDenylist`; reads `readIdentity`, `readInvestorCount`, `readIsDenied`, `readTokenBalance`. (Mint is the "exercise" proof surface — a compliance rejection surfaces at prepare/sign and is shown as a reason.)

- [ ] **Step 1: hub.ts console wrappers** — append to `web/src/hub.ts`

All mutating calls go through `signSendPoll` (imported from `stellar.ts`); reads use `server.simulateTransaction`. `attestCountry` targets the token's own identity instance (`hub.identity(token)`), which the issuer administers.
```ts
import { Contract, scValToNative as toNative } from '@stellar/stellar-sdk';
import { signSendPoll } from './stellar';

const HUB = hub.hub;
const scAddr = (a: string) => addr(a) as unknown as xdr.ScVal;

// Read-only simulation needs a validly-formatted ed25519 (G-) source account; the RPC does not
// require it to be funded. The all-zero "null" account is the standard placeholder. (A contract
// C-address would be rejected by `new Account`, which validates an ed25519 public key.)
const SIM_SOURCE = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF5';
async function sim(contractId: string, method: string, args: xdr.ScVal[]) {
  const acc = new Account(SIM_SOURCE, '0');
  const tx = buildFrom(acc, contractId, method, args as unknown as ReturnType<typeof addr>[]);
  return server.simulateTransaction(tx);
}

async function invoke(from: string, contractId: string, method: string, args: xdr.ScVal[], sign: SignFn, step: string) {
  const acc = await server.getAccount(from);
  const tx = buildFrom(acc, contractId, method, args as unknown as ReturnType<typeof addr>[]);
  return signSendPoll(tx, sign, step);
}

export const mint = (from: string, token: string, to: string, amount: string, sign: SignFn) =>
  invoke(from, token, 'mint', [scAddr(to), i128(amount) as unknown as xdr.ScVal], sign, 'mint');

export async function readIdentity(token: string): Promise<string> {
  const s = await sim(HUB, 'identity', [scAddr(token)]);
  if (rpc.Api.isSimulationError(s)) throw new Error('no identity for this token');
  return toNative(s.result!.retval) as string;
}
export async function attestCountry(from: string, token: string, account: string, code: number, sign: SignFn) {
  const identity = await readIdentity(token);
  return invoke(from, identity, 'set_country', [scAddr(account), u32(code)], sign, 'set_country');
}

export const setInvestorCap = (from: string, token: string, cap: number, sign: SignFn) =>
  invoke(from, HUB, 'set_investor_cap', [scAddr(token), u32(cap)], sign, 'set_investor_cap');
export const setMaxBalance = (from: string, token: string, cap: string, sign: SignFn) =>
  invoke(from, HUB, 'set_max_balance', [scAddr(token), i128(cap) as unknown as xdr.ScVal], sign, 'set_max_balance');
export const setMaxHolders = (from: string, token: string, cap: number, sign: SignFn) =>
  invoke(from, HUB, 'set_max_holders', [scAddr(token), u32(cap)], sign, 'set_max_holders');
export const pauseToken = (from: string, token: string, sign: SignFn) =>
  invoke(from, HUB, 'pause', [scAddr(token)], sign, 'pause');
export const unpauseToken = (from: string, token: string, sign: SignFn) =>
  invoke(from, HUB, 'unpause', [scAddr(token)], sign, 'unpause');
export const addToDenylist = (from: string, token: string, account: string, sign: SignFn) =>
  invoke(from, HUB, 'add_to_denylist', [scAddr(token), scAddr(account)], sign, 'add_to_denylist');

export async function readInvestorCount(token: string, country: number): Promise<string> {
  const s = await sim(HUB, 'investor_count', [scAddr(token), u32(country)]);
  return rpc.Api.isSimulationError(s) ? '—' : String(toNative(s.result!.retval));
}
export async function readIsDenied(token: string, account: string): Promise<boolean> {
  const s = await sim(HUB, 'is_denied', [scAddr(token), scAddr(account)]);
  return rpc.Api.isSimulationError(s) ? false : toNative(s.result!.retval) === true;
}
export async function readTokenBalance(token: string, account: string): Promise<string> {
  const s = await sim(token, 'balance', [scAddr(account)]);
  return rpc.Api.isSimulationError(s) ? '0' : String(toNative(s.result!.retval));
}
```
Add the missing imports at the top of `hub.ts` (`Contract` may be unused — omit if so; keep `Account`, `rpc` already imported). Ensure `xdr`, `u32`, `i128`, `addr`, `buildFrom`, `server`, `rpc`, `Account` are all in scope (import from `./stellar` / the SDK as needed). Run `npx tsc --noEmit` and drop any unused import it flags.

- [ ] **Step 2: TokenConsole UI** — `web/src/routes/TokenConsole.tsx`

Loads the record from `tokenStore`; renders only the sections whose module is active; each action shows its live result. The "Exercise" mint is the proof surface — a compliance rejection is caught at `signSendPoll`/prepare and shown as a reason.
```tsx
import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { useWallet } from '../wallet';
import { getToken } from '../tokenStore';
import {
  mint, attestCountry, readIdentity, readInvestorCount, readTokenBalance,
  setInvestorCap, pauseToken, unpauseToken, addToDenylist,
} from '../hub';

const EXPLORER = 'https://stellar.expert/explorer/testnet';

export function TokenConsole() {
  const { id = '' } = useParams();
  const { address, sign } = useWallet();
  const rec = address ? getToken(address, id) : undefined;
  const [msg, setMsg] = useState('');
  const [err, setErr] = useState('');
  const [mintTo, setMintTo] = useState('');
  const [mintAmt, setMintAmt] = useState('10');
  const [attAcct, setAttAcct] = useState('');
  const [attCode, setAttCode] = useState('840');
  const [identity, setIdentity] = useState('');
  const [bal, setBal] = useState('');

  const cfg = rec?.config;
  useEffect(() => { if (cfg && (cfg.country_restrict.length || cfg.max_investors)) readIdentity(id).then(setIdentity).catch(() => {}); }, [id, cfg]);

  if (!address) return <section className="card"><h2>Token console</h2><p>Connect your wallet.</p></section>;
  if (!rec || !cfg) return <section className="card"><h2>Token console</h2><p>Token not found in this browser. Launch one from <a href="/launch">the wizard</a>.</p></section>;

  const run = async (label: string, fn: () => Promise<string | void>) => {
    setMsg(`${label}…`); setErr('');
    try { const h = await fn(); setMsg(`${label} ✓${typeof h === 'string' ? ` (${h.slice(0,8)}…)` : ''}`); }
    catch (e) { setErr(`${label} rejected: ${String((e as Error).message || e)}`); setMsg(''); }
  };

  return (
    <section className="card">
      <h2>Token console</h2>
      <p><a href={`${EXPLORER}/contract/${id}`} target="_blank" rel="noreferrer">{id.slice(0,8)}…{id.slice(-6)}</a></p>
      <p className="muted">Active: {[
        cfg.denylist && 'denylist', cfg.max_balance !== '0' && 'max-balance',
        cfg.country_restrict.length && 'country-restrict', cfg.max_holders && 'max-holders',
        cfg.lockup && 'lockup', cfg.transfer_window && 'transfer-window', cfg.max_investors && 'max-investors',
      ].filter(Boolean).join(', ') || 'none'}</p>

      <h3>Mint</h3>
      <div className="field"><input placeholder="recipient G…" value={mintTo} onChange={(e) => setMintTo(e.target.value)} />
        <input type="number" value={mintAmt} onChange={(e) => setMintAmt(e.target.value)} />
        <button className="send" onClick={() => run('Mint', () => mint(address, id, mintTo, mintAmt, sign))}>Mint</button></div>

      {(cfg.country_restrict.length > 0 || cfg.max_investors > 0) && (
        <><h3>Attest identity</h3>
          <p className="muted">Identity: {identity ? `${identity.slice(0,8)}…` : '…'}</p>
          <div className="field"><input placeholder="account G…" value={attAcct} onChange={(e) => setAttAcct(e.target.value)} />
            <input value={attCode} onChange={(e) => setAttCode(e.target.value)} placeholder="ISO code e.g. 840" />
            <button className="send" onClick={() => run('Attest', () => attestCountry(address, id, attAcct, Number(attCode), sign))}>Attest country</button></div></>
      )}

      {cfg.max_investors > 0 && (
        <><h3>Max investors</h3>
          <button className="send" onClick={() => run('Set cap 2', () => setInvestorCap(address, id, 2, sign))}>Set per-country cap = 2</button>{' '}
          <button className="send" onClick={async () => setBal(await readInvestorCount(id, Number(attCode || 840)))}>Read count</button>
          {bal && <span className="pill">count({attCode})={bal}</span>}</>
      )}

      {cfg.transfer_window && (
        <><h3>Transfer window</h3>
          <button className="send" onClick={() => run('Pause', () => pauseToken(address, id, sign))}>Pause</button>{' '}
          <button className="send" onClick={() => run('Unpause', () => unpauseToken(address, id, sign))}>Unpause</button></>
      )}

      {cfg.denylist && (
        <><h3>Denylist</h3>
          <div className="field"><input placeholder="account G…" value={attAcct} onChange={(e) => setAttAcct(e.target.value)} />
            <button className="send" onClick={() => run('Denylist', () => addToDenylist(address, id, attAcct, sign))}>Add to denylist</button></div></>
      )}

      <h3>Read balance</h3>
      <div className="field"><input placeholder="account G…" value={mintTo} onChange={(e) => setMintTo(e.target.value)} />
        <button className="send" onClick={async () => setBal(await readTokenBalance(id, mintTo))}>Read</button>{bal && <span className="pill">{bal}</span>}</div>

      {msg && <div className="result">{msg}</div>}
      {err && <div className="result denied">{err}</div>}
    </section>
  );
}
```

- [ ] **Step 3: Typecheck, build, commit**

```bash
cd web && npx tsc --noEmit && npm run build
```
```bash
git add web/src/hub.ts web/src/routes/TokenConsole.tsx
git commit -m "feat(sp2): token console — mint/attest/manage/exercise, restrictions enforced live"
```

---

### Task 6: Live testnet verification + evidence

**Files:** Modify `docs/superpowers/specs/2026-07-17-constellation-launch-wizard-design.md` (append an Evidence section) or create `web/README-sp2.md`

- [ ] **Step 1: End-to-end run (dev server + real testnet)**

With `web/src/hub.testnet.json` populated (Task 1), run `cd web && npm run dev`. In the browser with Freighter on Testnet:
1. Connect wallet → `/launch`.
2. Configure `country_restrict: [US(840)]` + `max_investors: 1` → Review → Launch (ONE signature). Confirm redirect to `/token/:id` and the token appears on stellar.expert.
3. In the console: Attest two accounts as US(840) (two signatures on the token's identity). Mint 10 to the first (passes). Mint 10 to the second → the console shows the **per-country-cap rejection** (max-investors cap 1). Click "Set per-country cap = 2", then mint to the second again → passes; "Read count" shows 2.

If any step needs funded test accounts to mint to, generate them with `stellar keys generate <name> --network testnet --fund` and use their addresses.

- [ ] **Step 2: Record evidence**

Append a short "SP2 — live wizard launch" section (launch tx hash, token address, the console rejection screenshot-or-text, the cap-bump pass) to the design spec's end, mirroring the contract-layer evidence style. Commit:
```bash
git add docs/superpowers/specs/2026-07-17-constellation-launch-wizard-design.md
git commit -m "docs(sp2): live testnet evidence — wizard launch + console-enforced restriction"
```

- [ ] **Step 3: Final build gate**

```bash
cd web && npx tsc --noEmit && npm run build && node scripts/verify-launch-encoder.mjs
```
Expected: typecheck clean, build succeeds, golden ✅. SP2 complete: anyone can launch a real compliance token from the UI and watch its restrictions enforce live.
