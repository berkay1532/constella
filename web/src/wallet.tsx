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
