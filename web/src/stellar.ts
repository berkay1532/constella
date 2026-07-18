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
import deployed from './deployed.testnet.json';

export const server = new rpc.Server(deployed.rpcUrl);
export const NP = deployed.networkPassphrase;
const SOURCE = deployed.accounts.admin;

type ScVal = ReturnType<typeof nativeToScVal>;
export const addr = (a: string) => nativeToScVal(a, { type: 'address' });
export const i128 = (n: number | string) => nativeToScVal(n, { type: 'i128' });
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function buildFrom(sourceAcc: Account, contractId: string, method: string, args: ScVal[]) {
  const c = new Contract(contractId);
  return new TransactionBuilder(sourceAcc, { fee: BASE_FEE, networkPassphrase: NP })
    .addOperation(c.call(method, ...args))
    .setTimeout(120)
    .build();
}

// --- read-only simulation (no account needed beyond a placeholder) ---

async function simulate(contractId: string, method: string, args: ScVal[]) {
  return server.simulateTransaction(buildFrom(new Account(SOURCE, '0'), contractId, method, args));
}

export async function readBalance(account: string): Promise<string> {
  const sim = await simulate(deployed.contracts.token, 'balance', [addr(account)]);
  if (rpc.Api.isSimulationError(sim)) return '—';
  return String(scValToNative(sim.result!.retval));
}

export async function readTotalSupply(): Promise<string> {
  const sim = await simulate(deployed.contracts.token, 'total_supply', []);
  if (rpc.Api.isSimulationError(sim)) return '—';
  return String(scValToNative(sim.result!.retval));
}

export async function readHolders(): Promise<string> {
  const sim = await simulate(deployed.contracts.maxHolders, 'holders', []);
  if (rpc.Api.isSimulationError(sim)) return '—';
  return String(scValToNative(sim.result!.retval));
}

function humanize(msg: string): string {
  if (/#6\b|Denied/.test(msg)) return 'Denied by a compliance module';
  return msg;
}

const COUNTRY_NAMES: Record<number, string> = { 840: 'US', 276: 'DE', 792: 'TR' };

/**
 * Inspect the live identity/country contracts to explain *why* a transfer to `to`
 * is rejected (so the UI can show a specific, on-chain-verified reason). Returns ''
 * if it isn't a country/identity issue (some other module denied).
 */
export async function explainDenial(to: string): Promise<string> {
  try {
    const idSim = await simulate(deployed.contracts.identity, 'country_of', [addr(to)]);
    if (rpc.Api.isSimulationError(idSim)) return '';
    const code = scValToNative(idSim.result!.retval) as number | null | undefined;
    if (code === null || code === undefined) {
      return 'Recipient is not verified by the identity provider (checked on-chain)';
    }
    const allowSim = await simulate(deployed.contracts.countryRestrict, 'allowed', []);
    const allowed = rpc.Api.isSimulationError(allowSim)
      ? []
      : (scValToNative(allowSim.result!.retval) as number[]);
    if (!allowed.includes(Number(code))) {
      const name = COUNTRY_NAMES[Number(code)] ?? code;
      return `Recipient's country (${name}) is not in the allowed list — rejected by the on-chain CountryRestrict contract`;
    }
    return '';
  } catch {
    return '';
  }
}

export type SendResult = { ok: boolean; denied: boolean; reason: string; hash: string };

/**
 * Build + prepare + (Freighter-)sign + submit a real transfer from `from` to `to`.
 * `from` must be the connected wallet (it is both the tx source and the authorizer).
 * If a compliance module rejects, preparation fails and we report `denied`.
 */
export type SignFn = (xdr: string) => Promise<string>;
type ExplainFn = (to: string) => Promise<string>;

async function txTransfer(
  tokenId: string,
  from: string,
  to: string,
  amount: number,
  sign: SignFn,
  explain: ExplainFn,
): Promise<SendResult> {
  const account = await server.getAccount(from);
  const tx = buildFrom(account, tokenId, 'transfer', [addr(from), addr(to), i128(amount)]);

  let prepared;
  try {
    prepared = await server.prepareTransaction(tx);
  } catch (e) {
    // Compliance rejected at simulation (before signing) — explain it specifically.
    const specific = await explain(to).catch(() => '');
    return {
      ok: false,
      denied: true,
      reason: specific || humanize(String((e as Error).message || e)),
      hash: '',
    };
  }

  const signedXDR = await sign(prepared.toXDR());
  const signedTx = TransactionBuilder.fromXDR(signedXDR, NP);
  const sent = await server.sendTransaction(signedTx);
  if (sent.status === 'ERROR') {
    return { ok: false, denied: false, reason: 'Submit error', hash: sent.hash };
  }

  // Poll for finality, but tolerate result-meta XDR parse hiccups: the transaction is
  // already submitted, so fall back to "submitted" + explorer link rather than erroring.
  try {
    let got = await server.getTransaction(sent.hash);
    for (let i = 0; i < 20 && got.status === rpc.Api.GetTransactionStatus.NOT_FOUND; i++) {
      await sleep(1000);
      got = await server.getTransaction(sent.hash);
    }
    return {
      ok: got.status === rpc.Api.GetTransactionStatus.SUCCESS,
      denied: false,
      reason: got.status,
      hash: sent.hash,
    };
  } catch {
    return { ok: true, denied: false, reason: 'submitted', hash: sent.hash };
  }
}

/** Real signed transfer on the main (cleartext-compliance) token. */
export const submitTransfer = (from: string, to: string, amount: number, sign: SignFn) =>
  txTransfer(deployed.contracts.token, from, to, amount, sign, explainDenial);

// --- ZK eligibility (Phase 2) ---

type ZkInfo = { identityZk: string; commitment: string; zkToken: string; dave: string };
const ZK = (deployed as { zk?: ZkInfo }).zk;
export const hasZk = Boolean(ZK?.identityZk);
export const zkDave = ZK?.dave ?? '';

/** Read the ZK identity provider's eligibility flag for an account. */
export async function zkIsVerified(account: string): Promise<boolean> {
  if (!ZK) return false;
  const sim = await simulate(ZK.identityZk, 'is_verified', [addr(account)]);
  if (rpc.Api.isSimulationError(sim)) return false;
  return scValToNative(sim.result!.retval) === true;
}

/** Balance on the ZK-gated token. */
export async function readZkBalance(account: string): Promise<string> {
  if (!ZK) return '—';
  const sim = await simulate(ZK.zkToken, 'balance', [addr(account)]);
  if (rpc.Api.isSimulationError(sim)) return '—';
  return String(scValToNative(sim.result!.retval));
}

/** Real signed transfer on the ZK-gated token — gated on the recipient's ZK eligibility. */
export const submitZkTransfer = (from: string, to: string, amount: number, sign: SignFn) =>
  txTransfer(
    ZK!.zkToken,
    from,
    to,
    amount,
    sign,
    async () => 'Recipient is not ZK-eligible — their country is never revealed',
  );

// --- ZK eligibility: client-side register_self + prove_eligibility (Freighter-signed) ---

export const u256 = (dec: string) => nativeToScVal(BigInt(dec), { type: 'u256' });

// `xdr.ScVal.scvBytes` is typed as taking a Node `Buffer`, but this is a browser build with
// no `@types/node` (and no `Buffer` global) — the underlying js-xdr writer accepts any
// array-like of bytes at runtime, so pass the `Uint8Array` straight through and only cast
// the static type (via `Parameters<>` so we never have to name the unavailable `Buffer` type).
type ScvBytesArg = Parameters<typeof xdr.ScVal.scvBytes>[0];
const scvBytes = (v: Uint8Array) => xdr.ScVal.scvBytes(v as unknown as ScvBytesArg);

export function proofScVal(a: Uint8Array, b: Uint8Array, c: Uint8Array) {
  const entry = (k: string, v: xdr.ScVal) =>
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol(k), val: v });
  // A #[contracttype] struct serializes as a symbol-keyed ScMap sorted by key (a,b,c).
  return xdr.ScVal.scvMap([entry('a', scvBytes(a)), entry('b', scvBytes(b)), entry('c', scvBytes(c))]);
}

export async function signSendPoll(
  unsignedTx: import('@stellar/stellar-sdk').Transaction,
  sign: SignFn,
  step: string,
): Promise<string> {
  const prepared = await server.prepareTransaction(unsignedTx);
  const signedXDR = await sign(prepared.toXDR());
  const sent = await server.sendTransaction(TransactionBuilder.fromXDR(signedXDR, NP));
  if (sent.status === 'ERROR') throw new Error(`${step} submit error`);

  // Poll to finality. Tolerate result-meta XDR parse hiccups exactly as `txTransfer` does: the
  // tx is already submitted, so a parse exception is treated as "submitted" (hash returned) rather
  // than a failure. Only a definitive non-SUCCESS status (FAILED, or NOT_FOUND after 20×1s) throws —
  // so a doomed `register_self` aborts here, before we ask the wallet to sign `prove_eligibility`.
  try {
    let got = await server.getTransaction(sent.hash);
    for (let i = 0; i < 20 && got.status === rpc.Api.GetTransactionStatus.NOT_FOUND; i++) {
      await sleep(1000);
      got = await server.getTransaction(sent.hash);
    }
    if (got.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
      throw new Error(`${step} did not succeed: ${got.status}`);
    }
  } catch (e) {
    // Distinguish our own definitive-failure throw (re-throw it) from a result-meta parse hiccup
    // (treat as submitted, return the hash) — mirroring `txTransfer`'s tolerance.
    if (e instanceof Error && / did not succeed: /.test(e.message)) throw e;
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
  onStep?: (phase: 'register' | 'prove') => void,
): Promise<{ ok: boolean; registerHash: string; proveHash: string }> {
  if (!ZK) throw new Error('ZK not deployed');
  onStep?.('register');
  const acc1 = await server.getAccount(account);
  const registerTx = buildFrom(acc1, ZK.identityZk, 'register_self', [addr(account), u256(commitmentDec)]);
  const registerHash = await signSendPoll(registerTx, sign, 'register_self');

  onStep?.('prove');
  const acc2 = await server.getAccount(account);
  const proveTx = buildFrom(acc2, ZK.identityZk, 'prove_eligibility', [
    addr(account),
    u256(commitmentDec),
    proofScVal(proof.a, proof.b, proof.c),
  ]);
  const proveHash = await signSendPoll(proveTx, sign, 'prove_eligibility');

  const ok = await zkIsVerified(account);
  return { ok, registerHash, proveHash };
}

export { deployed };
