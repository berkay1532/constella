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
