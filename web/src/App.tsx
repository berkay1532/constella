import { useEffect, useState } from 'react';
import {
  deployed,
  readBalance,
  readTotalSupply,
  readHolders,
  submitTransfer,
  type SendResult,
} from './stellar';
import { connectWallet, currentAddress, signXDR } from './freighter';

const EXPLORER = 'https://stellar.expert/explorer/testnet';
const cLink = (id: string) => `${EXPLORER}/contract/${id}`;
const txLink = (h: string) => `${EXPLORER}/tx/${h}`;
const short = (a: string) => `${a.slice(0, 4)}…${a.slice(-4)}`;

const MODULES = [
  { name: 'CountryRestrict', id: deployed.contracts.countryRestrict, desc: 'Allowed countries: US, DE', kind: 'identity' },
  { name: 'MaxHolders', id: deployed.contracts.maxHolders, desc: 'Holder cap: 5', kind: 'trustless' },
  { name: 'MaxBalance', id: deployed.contracts.maxBalance, desc: 'Per-holder cap: 1,000,000', kind: 'trustless' },
  { name: 'Lockup', id: deployed.contracts.lockup, desc: 'Transfer lock: 0s (demo)', kind: 'trustless' },
];

const RECIPIENTS = [
  { addr: deployed.accounts.bob, label: 'Bob', flag: '🇩🇪', note: 'DE — allowed' },
  { addr: deployed.accounts.carol, label: 'Carol', flag: '🇹🇷', note: 'TR — not allowed' },
];

export function App() {
  const [supply, setSupply] = useState('…');
  const [holders, setHolders] = useState('…');
  const [wallet, setWallet] = useState<string | null>(null);
  const [walletBal, setWalletBal] = useState('—');
  const [busy, setBusy] = useState('');
  const [result, setResult] = useState<(SendResult & { to: string }) | null>(null);
  const [error, setError] = useState('');

  async function refresh() {
    setSupply(await readTotalSupply());
    setHolders(await readHolders());
    if (wallet) setWalletBal(await readBalance(wallet));
  }

  useEffect(() => {
    currentAddress().then((a) => setWallet(a));
  }, []);
  useEffect(() => {
    refresh().catch(console.error);
  }, [wallet]);

  async function onConnect() {
    setError('');
    try {
      setWallet(await connectWallet());
    } catch (e) {
      setError(String((e as Error).message || e));
    }
  }

  async function onPrepare() {
    if (!wallet) return;
    setError('');
    setBusy('prepare');
    try {
      const r = await fetch(`/api/bootstrap?addr=${wallet}`);
      const j = await r.json();
      if (!j.ok) throw new Error(j.error || 'bootstrap failed');
      await refresh();
    } catch (e) {
      setError(String((e as Error).message || e));
    } finally {
      setBusy('');
    }
  }

  async function onSend(to: string) {
    if (!wallet) return;
    setError('');
    setResult(null);
    setBusy(to);
    try {
      const res = await submitTransfer(wallet, to, 100, (xdr) => signXDR(xdr, wallet));
      setResult({ ...res, to });
      await refresh();
    } catch (e) {
      setError(String((e as Error).message || e));
    } finally {
      setBusy('');
    }
  }

  const needsTokens = wallet && walletBal !== '—' && Number(walletBal) === 0;

  return (
    <div className="wrap">
      <header>
        <h1>✨ Constella</h1>
        <p className="tag">Modular compliance for Stellar RWA tokens — live on testnet, signed with your wallet.</p>
      </header>

      <section className="card">
        <h2>Compliant token</h2>
        <div className="row">
          <span className="muted">Token</span>
          <a href={cLink(deployed.contracts.token)} target="_blank" rel="noreferrer">{short(deployed.contracts.token)} ↗</a>
        </div>
        <div className="stats">
          <div><b>{supply}</b><span>total supply</span></div>
          <div><b>{holders}</b><span>holders</span></div>
          <div><b>{MODULES.length}</b><span>modules</span></div>
        </div>
      </section>

      <section className="card">
        <h2>Registered compliance modules</h2>
        <ul className="modules">
          {MODULES.map((m) => (
            <li key={m.id}>
              <span className={`pill ${m.kind}`}>{m.kind === 'identity' ? 'identity' : 'trustless'}</span>
              <a href={cLink(m.id)} target="_blank" rel="noreferrer"><b>{m.name}</b></a>
              <span className="muted"> — {m.desc}</span>
            </li>
          ))}
        </ul>
      </section>

      <section className="card">
        <h2>Your wallet</h2>
        {!wallet ? (
          <>
            <p className="muted">Connect Freighter (testnet) to hold the regulated token and send real, signed transfers.</p>
            <button onClick={onConnect}>Connect Freighter</button>
          </>
        ) : (
          <>
            <div className="row">
              <span className="who">👛 {short(wallet)} <span className="muted">(US — attested)</span></span>
              <span className="bal">{walletBal} TOK</span>
            </div>
            {needsTokens && (
              <>
                <p className="muted">Your wallet isn’t set up yet. Prepare it: fund + attest country (US) + mint 1,000 TOK.</p>
                <button onClick={onPrepare} disabled={busy === 'prepare'}>
                  {busy === 'prepare' ? 'Preparing…' : 'Prepare my wallet'}
                </button>
              </>
            )}
          </>
        )}
      </section>

      {wallet && !needsTokens && (
        <section className="card">
          <h2>Send a real transfer (signed with Freighter)</h2>
          <p className="muted">100 TOK from your wallet. The compliance modules decide — and you sign the ones they allow.</p>
          <div className="send">
            {RECIPIENTS.map((r) => (
              <button key={r.addr} onClick={() => onSend(r.addr)} disabled={!!busy}>
                {busy === r.addr ? 'Checking on-chain…' : `Send 100 → ${r.flag} ${r.label}`}
                <span className="sub">{r.note}</span>
              </button>
            ))}
          </div>
          {result && (
            <div className={`result ${result.ok ? 'ok' : 'denied'}`}>
              {result.ok ? '✅ Transfer succeeded' : result.denied ? `❌ ${result.reason}` : `⚠️ ${result.reason}`}
              {result.hash && (
                <>
                  {' '}— <a href={txLink(result.hash)} target="_blank" rel="noreferrer">view tx ↗</a>
                </>
              )}
            </div>
          )}
          <p className="hint">
            Bob (DE) succeeds — you sign and it settles on-chain. Carol (TR) is rejected by the live
            CountryRestrict contract <b>at simulation, before you sign</b>: a compliant wallet won’t
            make you sign (or pay for) a transfer the on-chain rules reject — and Soroban needs a
            successful simulation to even build the transaction.
          </p>
        </section>
      )}

      {error && <div className="result denied">{error}</div>}

      <footer>
        <a href={cLink(deployed.contracts.compliance)} target="_blank" rel="noreferrer">compliance engine</a>
        {' · '}
        <a href={cLink(deployed.contracts.identity)} target="_blank" rel="noreferrer">identity provider</a>
        {' · '}network: {deployed.network}
      </footer>
    </div>
  );
}
