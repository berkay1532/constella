import {
  isConnected,
  requestAccess,
  getAddress,
  signTransaction,
} from '@stellar/freighter-api';
import { deployed } from './stellar';

/** Prompt the user to connect Freighter; returns the selected address. */
export async function connectWallet(): Promise<string> {
  const conn = await isConnected();
  if (!conn.isConnected) {
    throw new Error('Freighter not detected — install the browser extension and reload.');
  }
  const res = await requestAccess();
  if ((res as { error?: unknown }).error) throw new Error(String((res as { error: unknown }).error));
  return res.address;
}

/** Returns the already-authorized address, or null if not connected/allowed. */
export async function currentAddress(): Promise<string | null> {
  try {
    const r = await getAddress();
    return r.address || null;
  } catch {
    return null;
  }
}

/** Sign a transaction XDR with Freighter; returns the signed XDR. */
export async function signXDR(xdr: string, address: string): Promise<string> {
  const res = await signTransaction(xdr, {
    networkPassphrase: deployed.networkPassphrase,
    address,
  });
  if ((res as { error?: unknown }).error) throw new Error(String((res as { error: unknown }).error));
  return (res as { signedTxXdr: string }).signedTxXdr;
}
