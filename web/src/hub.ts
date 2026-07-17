import { xdr, nativeToScVal, scValToNative, TransactionBuilder, rpc, Account } from '@stellar/stellar-sdk';
import { server, NP, buildFrom, addr, i128, signSendPoll, deployed, type SignFn } from './stellar';
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
  let got;
  try {
    got = await server.getTransaction(sent.hash);
    for (let i = 0; i < 30 && got.status === rpc.Api.GetTransactionStatus.NOT_FOUND; i++) {
      await sleep(1000);
      got = await server.getTransaction(sent.hash);
    }
  } catch (e) {
    throw new Error(`launch submitted (tx ${sent.hash}) but the result could not be read: ${String((e as Error).message || e)}`);
  }
  if (got.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`launch did not succeed: ${got.status} (tx ${sent.hash})`);
  }
  const result = scValToNative(got.returnValue!) as { token: string };
  return { token: result.token, hash: sent.hash };
}

// --- Token console: mint, attest identity, manage caps/pause/denylist, and read live state. ---

const HUB = hub.hub;
const scAddr = (a: string) => addr(a) as unknown as xdr.ScVal;

// Read-only simulation needs a validly-formatted, existing ed25519 (G-) source account; the RPC
// does not require it to be the caller or even funded for a read-only sim. Reuse the already-valid,
// already-funded admin account that `stellar.ts` uses for its own simulations.
const SIM_SOURCE = deployed.accounts.admin;
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
  return scValToNative(s.result!.retval) as string;
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
  return rpc.Api.isSimulationError(s) ? '—' : String(scValToNative(s.result!.retval));
}
export async function readIsDenied(token: string, account: string): Promise<boolean> {
  const s = await sim(HUB, 'is_denied', [scAddr(token), scAddr(account)]);
  return rpc.Api.isSimulationError(s) ? false : scValToNative(s.result!.retval) === true;
}
export async function readTokenBalance(token: string, account: string): Promise<string> {
  const s = await sim(token, 'balance', [scAddr(account)]);
  return rpc.Api.isSimulationError(s) ? '0' : String(scValToNative(s.result!.retval));
}
