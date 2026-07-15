# In-browser ZK Proving Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate the Groth16 country-eligibility proof in the browser (country stays private) and submit eligibility with the user's Freighter wallet — no server, no admin — on the live testnet demo.

**Architecture:** Add a wallet-authed `register_self` to the ZK identity contract so a holder registers their own commitment; port the Rust proof→bytes encoder to TypeScript (verified byte-for-byte against the Rust output); run `snarkjs.groth16.fullProve` in the browser with the deployed circuit's `.wasm`/`.zkey`; submit `register_self` + `prove_eligibility` via Freighter using the existing stellar-sdk transaction pattern.

**Tech Stack:** Rust/soroban-sdk 26 (contract), snarkjs (browser proving), @stellar/stellar-sdk + Freighter (submission), React/Vite (UI), Node (golden-reference encoder test).

## Global Constraints

- Everything committed is in English (code, comments, commits, docs).
- Contracts build to `wasm32v1-none`; `cargo test --workspace` and `cargo clippy` stay green.
- Never regenerate the `.zkey` independently — always serve the exact artifact whose VK is deployed on-chain (`zk/build/country_eligibility_final.zkey`).
- Circuit is fixed: N=2, `allowed = [840, 276]` public; `country`, `secret` private; `commitment` is public signal index 0.
- TDD for the contract change; golden-reference verification for the encoder before any on-chain submission.
- Scope ends at `is_verified(account) == true`. ZK-gated token mint/transfer is out of scope.

---

### Task 1: Contract — `register_self` self-attestation (TDD)

**Files:**
- Modify: `crates/module-identity-zk/src/lib.rs`
- Test: `crates/module-identity-zk/src/test.rs`

**Interfaces:**
- Produces: `register_self(env: Env, account: Address, commitment: U256)` — sets `Commitment(account) = commitment` after `account.require_auth()`. Callable by anyone for an address they control.

- [ ] **Step 1: Write the failing tests**

Append to `crates/module-identity-zk/src/test.rs`:

```rust
#[test]
fn self_registration_then_prove() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = setup(&env);

    // Holder self-registers their own commitment (no admin), then proves eligibility.
    client.register_self(&investor, &commitment(&env));
    let ok = client.prove_eligibility(&investor, &commitment(&env), &proof(&env));
    assert_eq!(ok, true);
    assert_eq!(client.is_verified(&investor), true);
}

#[test]
#[should_panic]
fn register_self_requires_account_auth() {
    let env = Env::default();
    // No mock_all_auths: account.require_auth() must reject the call.
    let admin = Address::generate(&env);
    let verifier = env.register(Groth16Verifier {}, ());
    let id = env.register(IdentityZk, (admin, verifier));
    let client = IdentityZkClient::new(&env, &id);
    let investor = Address::generate(&env);
    client.register_self(&investor, &U256::from_u32(&env, 42));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p constella-module-identity-zk 2>&1 | tail -20`
Expected: FAIL — `register_self` does not exist (compile error / method missing).

- [ ] **Step 3: Implement `register_self`**

In `crates/module-identity-zk/src/lib.rs`, add this method inside the `#[contractimpl] impl IdentityZk` block, immediately after `register_commitment`:

```rust
    /// Demo self-attestation: the holder registers their own commitment (wallet-authed).
    /// Production keeps issuer attestation via `register_commitment`.
    pub fn register_self(env: Env, account: Address, commitment: U256) {
        account.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Commitment(account), &commitment);
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p constella-module-identity-zk 2>&1 | tail -8`
Expected: PASS — all tests including `self_registration_then_prove` and `register_self_requires_account_auth`.

- [ ] **Step 5: Lint + wasm build**

Run: `cargo clippy -p constella-module-identity-zk --all-targets 2>&1 | grep -E "warning|error" | grep -v "zk-verifier" | head`
Expected: no new warnings from this crate.
Run: `cargo build -p constella-module-identity-zk --target wasm32v1-none --release 2>&1 | tail -1`
Expected: `Finished ... release`.

- [ ] **Step 6: Commit**

```bash
git add crates/module-identity-zk/src/lib.rs crates/module-identity-zk/src/test.rs
git commit -m "feat(zk): register_self — wallet-authed self-attestation for demo proving"
```

---

### Task 2: Serve the proving artifacts + capture the encoder golden fixture

**Files:**
- Create: `web/public/zk/country_eligibility.wasm` (copied)
- Create: `web/public/zk/country_eligibility_final.zkey` (copied)
- Create: `web/src/zk/golden.json` (Rust encoder output, the fixture the TS encoder is checked against)

**Interfaces:**
- Produces: static assets served at `/zk/country_eligibility.wasm` and `/zk/country_eligibility_final.zkey`; `web/src/zk/golden.json` with shape `{ proof: { a, b, c }, vk: {...}, signals: [...], commitment_dec }` (hex strings).

- [ ] **Step 1: Ensure the circuit artifacts exist**

Run: `ls -lh zk/build/country_eligibility_js/country_eligibility.wasm zk/build/country_eligibility_final.zkey`
Expected: both files exist (~1.7M wasm, ~364K zkey). If missing, run `cd zk && bash build.sh && cd ..` first.

- [ ] **Step 2: Copy artifacts into the web public dir**

```bash
mkdir -p web/public/zk
cp zk/build/country_eligibility_js/country_eligibility.wasm web/public/zk/
cp zk/build/country_eligibility_final.zkey web/public/zk/
ls -lh web/public/zk/
```

- [ ] **Step 3: Capture the Rust encoder output as the golden fixture**

Run:
```bash
cargo run --manifest-path tools/zk-encode/Cargo.toml --quiet > web/src/zk/golden.json
python3 -c "import json;d=json.load(open('web/src/zk/golden.json'));print('keys:',list(d.keys()));print('a len:',len(d['proof']['a']),'b len:',len(d['proof']['b']),'c len:',len(d['proof']['c']))"
```
Expected: `keys: ['proof', 'vk', 'signals', 'commitment_dec']`; `a len: 192 b len: 384 c len: 192` (hex chars = 2× bytes: 96/192/96).

- [ ] **Step 4: Commit**

```bash
git add web/public/zk/country_eligibility.wasm web/public/zk/country_eligibility_final.zkey web/src/zk/golden.json
git commit -m "chore(zk): serve proving artifacts in web + capture encoder golden fixture"
```

---

### Task 3: TypeScript proof encoder + golden-reference test

**Files:**
- Create: `web/src/zk/encode.ts`
- Create: `web/scripts/verify-encoder.mjs`

**Interfaces:**
- Consumes: a snarkjs proof object `{ pi_a: [x,y,z], pi_b: [[x0,x1],[y0,y1],[z..]], pi_c: [x,y,z] }` (decimal strings); `web/src/zk/golden.json` from Task 2.
- Produces: `encodeProof(p): { a: Uint8Array(96), b: Uint8Array(192), c: Uint8Array(96) }` and `toHex(u8): string`.

- [ ] **Step 1: Write the encoder**

Create `web/src/zk/encode.ts`:

```ts
// TypeScript port of tools/zk-encode (arkworks uncompressed BLS12-381 layout).
// Field elements are canonical little-endian; Fr is emitted big-endian; points are
// x||y (G1) and x.c0||x.c1||y.c0||y.c1 (G2), matching the Rust encoder field order.

function leBytes(x: bigint, n: number): Uint8Array {
  const out = new Uint8Array(n);
  let v = x;
  for (let i = 0; i < n; i++) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

const le48 = (dec: string): Uint8Array => leBytes(BigInt(dec), 48);

function concat(...parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

/** G1Affine uncompressed: x (48 LE) || y (48 LE) = 96 bytes. */
export function g1(x: string, y: string): Uint8Array {
  return concat(le48(x), le48(y));
}

/** G2Affine uncompressed: x.c0 || x.c1 || y.c0 || y.c1 (each 48 LE) = 192 bytes. */
export function g2(x0: string, x1: string, y0: string, y1: string): Uint8Array {
  return concat(le48(x0), le48(x1), le48(y0), le48(y1));
}

/** Fr big-endian, 32 bytes. */
export function fr(dec: string): Uint8Array {
  return leBytes(BigInt(dec), 32).reverse();
}

export function toHex(u8: Uint8Array): string {
  return Array.from(u8, (b) => b.toString(16).padStart(2, '0')).join('');
}

export interface SnarkProof {
  pi_a: string[];
  pi_b: string[][];
  pi_c: string[];
}

/** Encode a snarkjs proof into the { a, b, c } byte blobs prove_eligibility expects. */
export function encodeProof(p: SnarkProof): { a: Uint8Array; b: Uint8Array; c: Uint8Array } {
  return {
    a: g1(p.pi_a[0], p.pi_a[1]),
    b: g2(p.pi_b[0][0], p.pi_b[0][1], p.pi_b[1][0], p.pi_b[1][1]),
    c: g1(p.pi_c[0], p.pi_c[1]),
  };
}
```

- [ ] **Step 2: Write the golden-reference test script**

Create `web/scripts/verify-encoder.mjs`:

```js
// Verify the TS encoder matches the Rust tools/zk-encode output byte-for-byte.
// Keeps an inline copy of the three encode primitives (le48/g1/g2) so it runs under
// plain Node with no TypeScript loader — see the note after this script.
// Run: node web/scripts/verify-encoder.mjs
import { readFileSync } from 'node:fs';

const proof = JSON.parse(readFileSync(new URL('../../zk/data/proof.json', import.meta.url), 'utf8'));
const golden = JSON.parse(readFileSync(new URL('../src/zk/golden.json', import.meta.url), 'utf8'));

// Inline copy of the encoder (kept in sync with web/src/zk/encode.ts) to avoid a TS loader.
const leBytes = (x, n) => { const o = new Uint8Array(n); let v = x; for (let i=0;i<n;i++){o[i]=Number(v&0xffn);v>>=8n;} return o; };
const le48 = (d) => leBytes(BigInt(d), 48);
const cat = (...ps) => { const t = ps.reduce((n,p)=>n+p.length,0); const o=new Uint8Array(t); let f=0; for(const p of ps){o.set(p,f);f+=p.length;} return o; };
const g1 = (x,y) => cat(le48(x), le48(y));
const g2 = (a,b,c,d) => cat(le48(a), le48(b), le48(c), le48(d));
const hex = (u8) => Array.from(u8, b => b.toString(16).padStart(2,'0')).join('');

const a = hex(g1(proof.pi_a[0], proof.pi_a[1]));
const b = hex(g2(proof.pi_b[0][0], proof.pi_b[0][1], proof.pi_b[1][0], proof.pi_b[1][1]));
const c = hex(g1(proof.pi_c[0], proof.pi_c[1]));

const ok = a === golden.proof.a && b === golden.proof.b && c === golden.proof.c;
if (!ok) {
  console.error('MISMATCH');
  console.error('a', a === golden.proof.a, '\n  ts:', a, '\n  rs:', golden.proof.a);
  console.error('b', b === golden.proof.b);
  console.error('c', c === golden.proof.c);
  process.exit(1);
}
console.log('✅ TS encoder matches Rust golden (a/b/c byte-for-byte)');
```

> Note: the `.mjs` keeps an inline copy of the three encode primitives (`le48`/`g1`/`g2`) to avoid a TypeScript loader in plain Node. When you change `encode.ts`, mirror the primitives here. They are ~5 lines and covered by this exact-match test.

- [ ] **Step 3: Run the golden test to verify it passes**

Run: `node web/scripts/verify-encoder.mjs`
Expected: `✅ TS encoder matches Rust golden (a/b/c byte-for-byte)`.
If it prints MISMATCH, the byte layout is wrong — do NOT proceed; fix `le48`/`g1`/`g2` ordering until it matches (this is the guarantee the browser proof verifies on-chain).

- [ ] **Step 4: Commit**

```bash
git add web/src/zk/encode.ts web/scripts/verify-encoder.mjs
git commit -m "feat(zk): TS proof encoder + golden-reference test vs Rust zk-encode"
```

---

### Task 4: Browser proving module (snarkjs in Vite)

**Files:**
- Modify: `web/package.json` (add `snarkjs` dependency)
- Modify: `web/vite.config.ts` (bundler shims for snarkjs)
- Create: `web/src/zk/prove.ts`

**Interfaces:**
- Consumes: served `/zk/country_eligibility.wasm`, `/zk/country_eligibility_final.zkey`.
- Produces: `generateProof(country: number, secret: bigint): Promise<{ proof: SnarkProof; commitment: string }>` — throws `IneligibleError` when the country is not in the allowed set.

- [ ] **Step 1: Add snarkjs**

Run: `cd web && npm install snarkjs@^0.7.5 && cd ..`
Expected: `snarkjs` added to `web/package.json` dependencies.

- [ ] **Step 2: Configure Vite for snarkjs**

Modify `web/vite.config.ts` — change the final export to add browser shims snarkjs needs:

```ts
export default defineConfig({
  plugins: [react(), bootstrapPlugin()],
  define: { global: 'globalThis' },
  optimizeDeps: { include: ['snarkjs'] },
});
```

- [ ] **Step 3: Write the prover**

Create `web/src/zk/prove.ts`:

```ts
import { groth16 } from 'snarkjs';
import type { SnarkProof } from './encode';

const WASM_URL = '/zk/country_eligibility.wasm';
const ZKEY_URL = '/zk/country_eligibility_final.zkey';
// Must match the on-chain policy set via set_policy.
const ALLOWED = ['840', '276'];

export class IneligibleError extends Error {
  constructor() {
    super('Country is not in the allowed set');
    this.name = 'IneligibleError';
  }
}

/**
 * Generate a Groth16 proof in the browser that the (private) country is in the allowed
 * set. The country and secret never leave this function. Returns the proof plus the
 * commitment (public signal 0). Throws IneligibleError if the country is not allowed
 * (the witness is unsatisfiable).
 */
export async function generateProof(
  country: number,
  secret: bigint,
): Promise<{ proof: SnarkProof; commitment: string }> {
  const input = { country: String(country), secret: secret.toString(), allowed: ALLOWED };
  let result;
  try {
    result = await groth16.fullProve(input, WASM_URL, ZKEY_URL);
  } catch (e) {
    // An unsatisfiable witness (disallowed country) surfaces as an assert error.
    throw new IneligibleError();
  }
  return { proof: result.proof as SnarkProof, commitment: String(result.publicSignals[0]) };
}
```

- [ ] **Step 4: Verify snarkjs proving runs in the browser (integration checkpoint)**

Run: `cd web && npm run dev` (starts Vite). In the browser console on the app page, run:
```js
const { generateProof } = await import('/src/zk/prove.ts');
const r = await generateProof(840, 1234567890n);
console.log('commitment', r.commitment, 'pi_a', r.proof.pi_a.length);
```
Expected: resolves with a `commitment` decimal string and `pi_a` length 3. Then verify the denial path:
```js
try { await generateProof(792, 1n); console.log('UNEXPECTED PASS'); }
catch (e) { console.log('denied as expected:', e.name); }
```
Expected: `denied as expected: IneligibleError`.

> If snarkjs fails to load/run under Vite (Node builtins, worker): apply the fallback — wrap proving in a Web Worker, or add `vite-plugin-node-polyfills`. Time-box ~half a day; do not proceed to Task 5 until this checkpoint passes.

- [ ] **Step 5: Commit**

```bash
git add web/package.json web/package-lock.json web/vite.config.ts web/src/zk/prove.ts
git commit -m "feat(zk): in-browser Groth16 proving via snarkjs (country stays client-side)"
```

---

### Task 5: Client-side submission via Freighter

**Files:**
- Modify: `web/src/stellar.ts` (add `submitZkEligibility`)

**Interfaces:**
- Consumes: `encodeProof` (Task 3), `generateProof` output; existing `server`, `buildFrom`, `NP`, `addr`, `SignFn`.
- Produces: `submitZkEligibility(account, commitment, proofBytes, sign): Promise<{ ok: boolean; registerHash: string; proveHash: string }>`.

- [ ] **Step 1: Add scval helpers + the submit function**

In `web/src/stellar.ts`, add `xdr` to the existing `@stellar/stellar-sdk` import (`Account` is already imported — see Step 1's import block at the end of this task), then add near the other exports:

```ts
// --- ZK eligibility: client-side register_self + prove_eligibility (Freighter-signed) ---

const u256 = (dec: string) => nativeToScVal(BigInt(dec), { type: 'u256' });

function proofScVal(a: Uint8Array, b: Uint8Array, c: Uint8Array) {
  const entry = (k: string, v: xdr.ScVal) =>
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol(k), val: v });
  // A #[contracttype] struct serializes as a symbol-keyed ScMap sorted by key (a,b,c).
  return xdr.ScVal.scvMap([
    entry('a', xdr.ScVal.scvBytes(Buffer.from(a))),
    entry('b', xdr.ScVal.scvBytes(Buffer.from(b))),
    entry('c', xdr.ScVal.scvBytes(Buffer.from(c))),
  ]);
}

async function signSendPoll(unsignedTx: import('@stellar/stellar-sdk').Transaction, sign: SignFn): Promise<string> {
  const prepared = await server.prepareTransaction(unsignedTx);
  const signedXDR = await sign(prepared.toXDR());
  const sent = await server.sendTransaction(TransactionBuilder.fromXDR(signedXDR, NP));
  if (sent.status === 'ERROR') throw new Error('submit error');
  let got = await server.getTransaction(sent.hash);
  for (let i = 0; i < 20 && got.status === rpc.Api.GetTransactionStatus.NOT_FOUND; i++) {
    await sleep(1000);
    got = await server.getTransaction(sent.hash);
  }
  return sent.hash;
}

/**
 * Register the holder's own commitment and submit the browser-generated proof, both signed
 * by the connected wallet. No admin/server involved. Returns the two tx hashes.
 */
export async function submitZkEligibility(
  account: string,
  commitmentDec: string,
  proof: { a: Uint8Array; b: Uint8Array; c: Uint8Array },
  sign: SignFn,
): Promise<{ ok: boolean; registerHash: string; proveHash: string }> {
  if (!ZK) throw new Error('ZK not deployed');
  const acc1 = await server.getAccount(account);
  const registerTx = buildFrom(acc1, ZK.identityZk, 'register_self', [addr(account), u256(commitmentDec)]);
  const registerHash = await signSendPoll(registerTx, sign);

  const acc2 = await server.getAccount(account);
  const proveTx = buildFrom(acc2, ZK.identityZk, 'prove_eligibility', [
    addr(account),
    u256(commitmentDec),
    proofScVal(proof.a, proof.b, proof.c),
  ]);
  const proveHash = await signSendPoll(proveTx, sign);

  const ok = await zkIsVerified(account);
  return { ok, registerHash, proveHash };
}
```

Update the top import line to include `xdr`:

```ts
import {
  rpc,
  Contract,
  TransactionBuilder,
  Account,
  nativeToScVal,
  scValToNative,
  BASE_FEE,
  xdr,
} from '@stellar/stellar-sdk';
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit 2>&1 | head -20`
Expected: no errors. Fix any type mismatches (e.g. `Transaction` import) before proceeding.

- [ ] **Step 3: Commit**

```bash
git add web/src/stellar.ts
git commit -m "feat(zk): submitZkEligibility — client-side register_self + prove via Freighter"
```

---

### Task 6: Rewire the ZK-eligibility UI card

**Files:**
- Modify: `web/src/App.tsx`

**Interfaces:**
- Consumes: `generateProof`, `IneligibleError` (Task 4), `encodeProof` (Task 3), `submitZkEligibility` (Task 5), `signXDR`.

- [ ] **Step 1: Wire the country-picker + browser-prove flow**

In `web/src/App.tsx`:

1. Add imports:
```ts
import { generateProof, IneligibleError } from './zk/prove';
import { encodeProof } from './zk/encode';
import { submitZkEligibility } from './stellar';
```
2. Add state near the other ZK state:
```ts
const [zkCountry, setZkCountry] = useState(840);
const [zkDenied, setZkDenied] = useState(false);
```
3. Add the country options constant near the other consts:
```ts
const ZK_COUNTRIES = [
  { code: 840, flag: '🇺🇸', label: 'United States (allowed)' },
  { code: 276, flag: '🇩🇪', label: 'Germany (allowed)' },
  { code: 792, flag: '🇹🇷', label: 'Turkey (not allowed)' },
  { code: 250, flag: '🇫🇷', label: 'France (not allowed)' },
];
```
4. Replace the `onProveZk` function body with the client-side flow:
```ts
  async function onProveZk() {
    if (!wallet) return;
    setError('');
    setZkDenied(false);
    setZkBusy(true);
    try {
      const secret = BigInt('0x' + crypto.getRandomValues(new Uint8Array(8)).reduce((s, b) => s + b.toString(16).padStart(2, '0'), ''));
      const { proof, commitment } = await generateProof(zkCountry, secret);
      const bytes = encodeProof(proof);
      const res = await submitZkEligibility(wallet, commitment, bytes, (xdr) => signXDR(xdr, wallet));
      setZkHash(res.proveHash);
      setZkVerified(res.ok);
    } catch (e) {
      if (e instanceof IneligibleError) {
        setZkDenied(true);
      } else {
        setError(String((e as Error).message || e));
      }
    } finally {
      setZkBusy(false);
    }
  }
```

- [ ] **Step 2: Update the card's JSX**

Replace the `!zkVerified ? (...)` button block in the ZK-eligibility `<section>` with a country picker + the denial message:

```tsx
          {!zkVerified ? (
            <>
              <div className="row">
                <span className="muted">Your country (private)</span>
                <select value={zkCountry} onChange={(e) => setZkCountry(Number(e.target.value))}>
                  {ZK_COUNTRIES.map((c) => (
                    <option key={c.code} value={c.code}>{c.flag} {c.label}</option>
                  ))}
                </select>
              </div>
              <button onClick={onProveZk} disabled={zkBusy}>
                {zkBusy ? 'Generating proof in your browser…' : 'Prove eligibility (zero-knowledge)'}
              </button>
              {zkDenied && (
                <div className="result denied">
                  ❌ Not eligible — and the app never learned or transmitted your country.
                </div>
              )}
            </>
          ) : (
            zkHash && (
              <div className="result ok">
                ✅ Proof generated in your browser & verified on-chain — country stayed private —{' '}
                <a href={txLink(zkHash)} target="_blank" rel="noreferrer">view tx ↗</a>
              </div>
            )
          )}
```

- [ ] **Step 3: Type-check + build**

Run: `cd web && npx tsc --noEmit 2>&1 | head -20 && npm run build 2>&1 | tail -5`
Expected: no type errors; `vite build` succeeds.

- [ ] **Step 4: Commit**

```bash
git add web/src/App.tsx
git commit -m "feat(zk): country picker + in-browser proving in the eligibility card"
```

---

### Task 7: Redeploy, verify end-to-end, document

**Files:**
- Modify: `scripts/deployed.testnet.json`, `web/src/deployed.testnet.json` (regenerated)
- Modify: `README.md` (note in-browser proving), `docs/evidence-testnet.md` if applicable

**Interfaces:**
- Consumes: all prior tasks.

- [ ] **Step 1: Redeploy the stack (picks up the new identity-zk wasm)**

Run: `bash scripts/deploy-testnet.sh 2>&1 | tail -20`
Expected: `✅ Done.` The redeployed `identityZk` now has `register_self`; `set_policy` unchanged.

- [ ] **Step 2: Sync the web deployment JSON**

Run: `cp scripts/deployed.testnet.json web/src/deployed.testnet.json`

- [ ] **Step 3: End-to-end browser verification**

Run: `cd web && npm run dev`. In the browser with Freighter (testnet):
1. Connect wallet.
2. In the ZK card pick 🇺🇸 US → "Prove eligibility" → approve the two Freighter prompts.
   Expected: ✅ verified, a `view tx ↗` link; `is_verified` true.
3. Reset (fresh wallet or re-check), pick 🇹🇷 Turkey → "Prove eligibility".
   Expected: ❌ "Not eligible…" with **no** wallet prompt and no tx (proof failed in-browser).

- [ ] **Step 4: Update docs**

In `README.md` section 6 (the ZK layer), add one line noting the eligibility proof is now generated **client-side in the browser** (country never leaves the device) and submitted with the user's wallet — no server. Keep the "production hardening is Instaward #2" framing.

- [ ] **Step 5: Commit**

```bash
git add scripts/deployed.testnet.json web/src/deployed.testnet.json README.md
git commit -m "feat(zk): redeploy with register_self + document in-browser proving"
```

- [ ] **Step 6: Full verification gate**

Run: `cargo test --workspace 2>&1 | grep -E "test result: FAIL|^error" || echo "workspace green"`
Run: `node web/scripts/verify-encoder.mjs`
Expected: `workspace green` and the encoder golden match. Open a PR when both pass.
