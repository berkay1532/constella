import {
  rpc,
  Contract,
  TransactionBuilder,
  Account,
  nativeToScVal,
  scValToNative,
  BASE_FEE,
} from '@stellar/stellar-sdk';
import deployed from './deployed.testnet.json';

const server = new rpc.Server(deployed.rpcUrl);
const NP = deployed.networkPassphrase;
// Any existing account works as the source for a read-only simulation; auth is
// recorded (not enforced) during simulateTransaction, so the compliance logic runs.
const SOURCE = deployed.accounts.admin;

function build(contractId: string, method: string, args: ReturnType<typeof nativeToScVal>[]) {
  const c = new Contract(contractId);
  const acc = new Account(SOURCE, '0');
  return new TransactionBuilder(acc, { fee: BASE_FEE, networkPassphrase: NP })
    .addOperation(c.call(method, ...args))
    .setTimeout(30)
    .build();
}

async function simulate(contractId: string, method: string, args: ReturnType<typeof nativeToScVal>[]) {
  return server.simulateTransaction(build(contractId, method, args));
}

const addr = (a: string) => nativeToScVal(a, { type: 'address' });
const i128 = (n: number | string) => nativeToScVal(n, { type: 'i128' });

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

export type TransferResult = { ok: boolean; reason: string };

/** Live-simulate a transfer against the deployed compliant token. */
export async function simulateTransfer(from: string, to: string, amount: number): Promise<TransferResult> {
  const sim = await simulate(deployed.contracts.token, 'transfer', [addr(from), addr(to), i128(amount)]);
  if (rpc.Api.isSimulationError(sim)) {
    // The dispatcher panics with ComplianceError::Denied (#6) when a module rejects.
    const denied = /Error\(Contract, #6\)|Denied/.test(sim.error);
    return { ok: false, reason: denied ? 'Denied by a compliance module' : sim.error };
  }
  return { ok: true, reason: 'Allowed by all registered modules' };
}

export { deployed };
