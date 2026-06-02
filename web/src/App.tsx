import { useEffect, useState } from 'react';
import {
  deployed,
  readBalance,
  readTotalSupply,
  readHolders,
  simulateTransfer,
  type TransferResult,
} from './stellar';

const EXPLORER = 'https://stellar.expert/explorer/testnet';
const cLink = (id: string) => `${EXPLORER}/contract/${id}`;

const PEOPLE: Record<string, { label: string; country: string; flag: string }> = {
  [deployed.accounts.alice]: { label: 'Alice', country: 'US (allowed)', flag: '🇺🇸' },
  [deployed.accounts.bob]: { label: 'Bob', country: 'DE (allowed)', flag: '🇩🇪' },
  [deployed.accounts.carol]: { label: 'Carol', country: 'TR (not allowed)', flag: '🇹🇷' },
};

const MODULES = [
  { name: 'CountryRestrict', id: deployed.contracts.countryRestrict, desc: 'Allowed countries only: US, DE', kind: 'identity' },
  { name: 'MaxHolders', id: deployed.contracts.maxHolders, desc: 'Holder cap: 5', kind: 'trustless' },
  { name: 'MaxBalance', id: deployed.contracts.maxBalance, desc: 'Per-holder cap: 1,000,000', kind: 'trustless' },
  { name: 'Lockup', id: deployed.contracts.lockup, desc: 'Transfer lock: 0s (demo)', kind: 'trustless' },
];

const short = (a: string) => `${a.slice(0, 4)}…${a.slice(-4)}`;

export function App() {
  const [supply, setSupply] = useState('…');
  const [holders, setHolders] = useState('…');
  const [balances, setBalances] = useState<Record<string, string>>({});
  const [from, setFrom] = useState(deployed.accounts.alice);
  const [to, setTo] = useState(deployed.accounts.carol);
  const [amount, setAmount] = useState(100);
  const [result, setResult] = useState<TransferResult | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    setSupply(await readTotalSupply());
    setHolders(await readHolders());
    const entries = await Promise.all(
      Object.keys(PEOPLE).map(async (a) => [a, await readBalance(a)] as const),
    );
    setBalances(Object.fromEntries(entries));
  }

  useEffect(() => {
    refresh().catch(console.error);
  }, []);

  async function onSimulate() {
    setBusy(true);
    setResult(null);
    try {
      setResult(await simulateTransfer(from, to, amount));
    } catch (e) {
      setResult({ ok: false, reason: String(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="wrap">
      <header>
        <h1>✨ Constella</h1>
        <p className="tag">Modular compliance for Stellar RWA tokens — live on testnet.</p>
      </header>

      <section className="card">
        <h2>Compliant token</h2>
        <div className="row">
          <span className="muted">Token</span>
          <a href={cLink(deployed.contracts.token)} target="_blank" rel="noreferrer">
            {short(deployed.contracts.token)} ↗
          </a>
        </div>
        <div className="stats">
          <div><b>{supply}</b><span>total supply</span></div>
          <div><b>{holders}</b><span>holders</span></div>
          <div><b>{MODULES.length}</b><span>modules</span></div>
        </div>
      </section>

      <section className="card">
        <h2>Registered compliance modules</h2>
        <p className="muted">Plug-and-play rules checked on every transfer by the dispatcher.</p>
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
        <h2>Investors (attested by the identity provider)</h2>
        <ul className="people">
          {Object.entries(PEOPLE).map(([addr, p]) => (
            <li key={addr}>
              <span className="who">{p.flag} {p.label}</span>
              <span className="muted">{p.country}</span>
              <span className="bal">{balances[addr] ?? '…'} TOK</span>
            </li>
          ))}
        </ul>
      </section>

      <section className="card">
        <h2>Try a transfer (live simulation)</h2>
        <p className="muted">Simulated against the deployed contracts on testnet — the real modules decide.</p>
        <div className="form">
          <label>From
            <select value={from} onChange={(e) => setFrom(e.target.value)}>
              {Object.entries(PEOPLE).map(([a, p]) => <option key={a} value={a}>{p.label}</option>)}
            </select>
          </label>
          <label>To
            <select value={to} onChange={(e) => setTo(e.target.value)}>
              {Object.entries(PEOPLE).map(([a, p]) => <option key={a} value={a}>{p.label}</option>)}
            </select>
          </label>
          <label>Amount
            <input type="number" value={amount} min={1} onChange={(e) => setAmount(Number(e.target.value))} />
          </label>
          <button onClick={onSimulate} disabled={busy}>{busy ? 'Simulating…' : 'Simulate transfer'}</button>
        </div>
        {result && (
          <div className={`result ${result.ok ? 'ok' : 'denied'}`}>
            {result.ok ? '✅ Allowed' : '❌ Denied'} — {result.reason}
          </div>
        )}
        <p className="hint">Tip: Alice→Bob passes; Alice→Carol is denied (Carol is in a disallowed country).
          In production the sender signs with a wallet (e.g. Freighter); here we simulate to show the gate.</p>
      </section>

      <footer>
        <a href={cLink(deployed.contracts.compliance)} target="_blank" rel="noreferrer">compliance engine</a>
        {' · '}
        <a href={cLink(deployed.contracts.identity)} target="_blank" rel="noreferrer">identity provider</a>
        {' · '}network: {deployed.network}
      </footer>
    </div>
  );
}
