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
const SOURCE = deployed.accounts.admin;

type ScVal = ReturnType<typeof nativeToScVal>;
const addr = (a: string) => nativeToScVal(a, { type: 'address' });
const i128 = (n: number | string) => nativeToScVal(n, { type: 'i128' });
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function buildFrom(sourceAcc: Account, contractId: string, method: string, args: ScVal[]) {
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

export type SendResult = { ok: boolean; denied: boolean; reason: string; hash: string };

/**
 * Build + prepare + (Freighter-)sign + submit a real transfer from `from` to `to`.
 * `from` must be the connected wallet (it is both the tx source and the authorizer).
 * If a compliance module rejects, preparation fails and we report `denied`.
 */
export async function submitTransfer(
  from: string,
  to: string,
  amount: number,
  sign: (xdr: string) => Promise<string>,
): Promise<SendResult> {
  const account = await server.getAccount(from);
  const tx = buildFrom(account, deployed.contracts.token, 'transfer', [addr(from), addr(to), i128(amount)]);

  let prepared;
  try {
    prepared = await server.prepareTransaction(tx);
  } catch (e) {
    return { ok: false, denied: true, reason: humanize(String((e as Error).message || e)), hash: '' };
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

export { deployed };
