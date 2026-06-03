import { useEffect, useRef, useState } from 'react';
import {
  deployed,
  readBalance,
  readTotalSupply,
  readHolders,
  submitTransfer,
  submitZkTransfer,
  readZkBalance,
  zkIsVerified,
  hasZk,
  zkDave,
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

const BASE_HOLDERS = [
  { addr: deployed.accounts.alice, label: 'Alice', flag: '🇺🇸', country: 'US' },
  { addr: deployed.accounts.bob, label: 'Bob', flag: '🇩🇪', country: 'DE' },
  { addr: deployed.accounts.carol, label: 'Carol', flag: '🇹🇷', country: 'TR' },
];

const ZK_RECIPIENTS = [
  { addr: zkDave, label: 'Dave', flag: '🟢', note: 'ZK-eligible (proven)' },
  { addr: deployed.accounts.carol, label: 'Carol', flag: '🔴', note: 'not ZK-eligible — country hidden' },
];

export function App() {
  const [supply, setSupply] = useState('…');
  const [holders, setHolders] = useState('…');
  const [wallet, setWallet] = useState<string | null>(null);
  const [busy, setBusy] = useState('');
  const [result, setResult] = useState<(SendResult & { to: string }) | null>(null);
  const [error, setError] = useState('');
  const [balances, setBalances] = useState<Record<string, string>>({});
  const [zkVerified, setZkVerified] = useState(false);
  const [zkBusy, setZkBusy] = useState(false);
  const [zkHash, setZkHash] = useState('');
  const [zkBal, setZkBal] = useState('—');
  const [zkBusy2, setZkBusy2] = useState('');
  const [zkSendRes, setZkSendRes] = useState<(SendResult & { to: string }) | null>(null);

  const holderRows = wallet
    ? [{ addr: wallet, label: 'You', flag: '👛', country: 'US' }, ...BASE_HOLDERS]
    : BASE_HOLDERS;
  const walletBal = wallet ? balances[wallet] ?? '—' : '—';

  const reqId = useRef(0);
  async function refresh() {
    // Guard against out-of-order responses: when the page loads with a wallet already
    // connected, an early wallet=null refresh races a later wallet-aware one. Only the
    // newest refresh's result is allowed to win, so it never clobbers the wallet balance.
    const myId = ++reqId.current;
    const supplyV = await readTotalSupply();
    const holdersV = await readHolders();
    const addrs = wallet ? [wallet, ...BASE_HOLDERS.map((h) => h.addr)] : BASE_HOLDERS.map((h) => h.addr);
    const entries = await Promise.all(addrs.map(async (a) => [a, await readBalance(a)] as const));
    if (myId !== reqId.current) return; // superseded by a newer refresh
    setSupply(supplyV);
    setHolders(holdersV);
    setBalances(Object.fromEntries(entries));
    if (wallet && hasZk) {
      setZkVerified(await zkIsVerified(wallet));
      setZkBal(await readZkBalance(wallet));
    }
  }

  async function onZkMint() {
    if (!wallet) return;
    setError('');
    setZkBusy2('mint');
    try {
      const r = await fetch(`/api/zk-mint?addr=${wallet}`);
      const j = await r.json();
      if (!j.ok) throw new Error(j.error || 'mint failed');
      await refresh();
    } catch (e) {
      setError(String((e as Error).message || e));
    } finally {
      setZkBusy2('');
    }
  }

  async function onZkSend(to: string) {
    if (!wallet) return;
    setError('');
    setZkSendRes(null);
    setZkBusy2(to);
    try {
      const res = await submitZkTransfer(wallet, to, 100, (xdr) => signXDR(xdr, wallet));
      setZkSendRes({ ...res, to });
      await refresh();
    } catch (e) {
      setError(String((e as Error).message || e));
    } finally {
      setZkBusy2('');
    }
  }

  async function onProveZk() {
    if (!wallet) return;
    setError('');
    setZkBusy(true);
    try {
      const r = await fetch(`/api/zk-prove?addr=${wallet}`);
      const j = await r.json();
      if (!j.ok) throw new Error(j.error || 'zk proof failed');
      setZkHash(j.hash || '');
      setZkVerified(await zkIsVerified(wallet));
    } catch (e) {
      setError(String((e as Error).message || e));
    } finally {
      setZkBusy(false);
    }
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
        <h2>Token holders</h2>
        <p className="muted">Live balances — updated after each transfer.</p>
        <table className="holders-table">
          <thead>
            <tr><th>Holder</th><th>Country</th><th>Balance</th></tr>
          </thead>
          <tbody>
            {holderRows.map((h) => (
              <tr key={h.addr}>
                <td>{h.flag} {h.label} <span className="muted">{short(h.addr)}</span></td>
                <td>{h.country}</td>
                <td className="bal">{balances[h.addr] ?? '…'} TOK</td>
              </tr>
            ))}
          </tbody>
        </table>
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

      {wallet && hasZk && (
        <section className="card">
          <h2>🔒 Zero-knowledge eligibility (Phase 2)</h2>
          <p className="muted">
            Prove your country is in the allowed set <b>without revealing which country</b> — a real
            Groth16/BLS12-381 proof, verified on-chain. The contract only ever sees a commitment and the
            allowed set; your country is never published.
          </p>
          <div className="row">
            <span className="muted">ZK status</span>
            <span className={zkVerified ? 'zk-ok' : 'muted'}>
              {zkVerified ? '✅ Proven eligible · country_of: none (private)' : 'not proven yet'}
            </span>
          </div>
          {!zkVerified ? (
            <button onClick={onProveZk} disabled={zkBusy}>
              {zkBusy ? 'Verifying proof on-chain…' : 'Prove eligibility (zero-knowledge)'}
            </button>
          ) : (
            zkHash && (
              <div className="result ok">
                ✅ Proof verified on-chain — country stayed private —{' '}
                <a href={txLink(zkHash)} target="_blank" rel="noreferrer">view tx ↗</a>
              </div>
            )
          )}
        </section>
      )}

      {wallet && hasZk && zkVerified && (
        <section className="card">
          <h2>🔒 ZK-gated transfer — recipient privacy</h2>
          <p className="muted">
            This token gates on the recipient's <b>ZK eligibility flag</b>, not a cleartext country.
            A non-eligible recipient is simply rejected — their country is never read or revealed.
          </p>
          <div className="row">
            <span className="muted">Your ZK-token balance</span>
            <span className="bal">{zkBal} zkTOK</span>
          </div>
          {zkBal !== '—' && Number(zkBal) === 0 ? (
            <button onClick={onZkMint} disabled={zkBusy2 === 'mint'}>
              {zkBusy2 === 'mint' ? 'Minting…' : 'Get ZK-gated tokens'}
            </button>
          ) : (
            <div className="send">
              {ZK_RECIPIENTS.map((r) => (
                <button key={r.addr} onClick={() => onZkSend(r.addr)} disabled={!!zkBusy2}>
                  {zkBusy2 === r.addr ? 'Checking on-chain…' : `Send 100 → ${r.flag} ${r.label}`}
                  <span className="sub">{r.note}</span>
                </button>
              ))}
            </div>
          )}
          {zkSendRes && (
            <div className={`result ${zkSendRes.ok ? 'ok' : 'denied'}`}>
              {zkSendRes.ok ? '✅ Transfer succeeded' : `❌ ${zkSendRes.reason}`}
              {zkSendRes.hash && (
                <>
                  {' '}— <a href={txLink(zkSendRes.hash)} target="_blank" rel="noreferrer">view tx ↗</a>
                </>
              )}
            </div>
          )}
          <p className="hint">
            Compare with the cleartext token above: there, rejecting a transfer to Carol revealed she is
            <b> TR</b>. Here, Carol is rejected as “not eligible” and her country never appears on-chain.
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
