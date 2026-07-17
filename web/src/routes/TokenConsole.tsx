import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { useWallet } from '../wallet';
import { getToken } from '../tokenStore';
import {
  mint, attestCountry, readIdentity, readInvestorCount, readTokenBalance,
  setInvestorCap, pauseToken, unpauseToken, addToDenylist,
} from '../hub';

const EXPLORER = 'https://stellar.expert/explorer/testnet';

export function TokenConsole() {
  const { id = '' } = useParams();
  const { address, sign } = useWallet();
  const rec = address ? getToken(address, id) : undefined;
  const [msg, setMsg] = useState('');
  const [err, setErr] = useState('');
  const [mintTo, setMintTo] = useState('');
  const [mintAmt, setMintAmt] = useState('10');
  const [attAcct, setAttAcct] = useState('');
  const [attCode, setAttCode] = useState('840');
  const [identity, setIdentity] = useState('');
  const [bal, setBal] = useState('');

  const cfg = rec?.config;
  useEffect(() => { if (cfg && (cfg.country_restrict.length || cfg.max_investors)) readIdentity(id).then(setIdentity).catch(() => {}); }, [id, cfg]);

  if (!address) return <section className="card"><h2>Token console</h2><p>Connect your wallet.</p></section>;
  if (!rec || !cfg) return <section className="card"><h2>Token console</h2><p>Token not found in this browser. Launch one from <a href="/launch">the wizard</a>.</p></section>;

  const run = async (label: string, fn: () => Promise<string | void>) => {
    setMsg(`${label}…`); setErr('');
    try { const h = await fn(); setMsg(`${label} ✓${typeof h === 'string' ? ` (${h.slice(0,8)}…)` : ''}`); }
    catch (e) { setErr(`${label} rejected: ${String((e as Error).message || e)}`); setMsg(''); }
  };

  return (
    <section className="card">
      <h2>Token console</h2>
      <p><a href={`${EXPLORER}/contract/${id}`} target="_blank" rel="noreferrer">{id.slice(0,8)}…{id.slice(-6)}</a></p>
      <p className="muted">Active: {[
        cfg.denylist && 'denylist', cfg.max_balance !== '0' && 'max-balance',
        cfg.country_restrict.length && 'country-restrict', cfg.max_holders && 'max-holders',
        cfg.lockup && 'lockup', cfg.transfer_window && 'transfer-window', cfg.max_investors && 'max-investors',
      ].filter(Boolean).join(', ') || 'none'}</p>

      <h3>Mint</h3>
      <div className="field"><input placeholder="recipient G…" value={mintTo} onChange={(e) => setMintTo(e.target.value)} />
        <input type="number" value={mintAmt} onChange={(e) => setMintAmt(e.target.value)} />
        <button className="send" onClick={() => run('Mint', () => mint(address, id, mintTo, mintAmt, sign))}>Mint</button></div>

      {(cfg.country_restrict.length > 0 || cfg.max_investors > 0) && (
        <><h3>Attest identity</h3>
          <p className="muted">Identity: {identity ? `${identity.slice(0,8)}…` : '…'}</p>
          <div className="field"><input placeholder="account G…" value={attAcct} onChange={(e) => setAttAcct(e.target.value)} />
            <input value={attCode} onChange={(e) => setAttCode(e.target.value)} placeholder="ISO code e.g. 840" />
            <button className="send" onClick={() => run('Attest', () => attestCountry(address, id, attAcct, Number(attCode), sign))}>Attest country</button></div></>
      )}

      {cfg.max_investors > 0 && (
        <><h3>Max investors</h3>
          <button className="send" onClick={() => run('Set cap 2', () => setInvestorCap(address, id, 2, sign))}>Set per-country cap = 2</button>{' '}
          <button className="send" onClick={async () => setBal(await readInvestorCount(id, Number(attCode || 840)))}>Read count</button>
          {bal && <span className="pill">count({attCode})={bal}</span>}</>
      )}

      {cfg.transfer_window && (
        <><h3>Transfer window</h3>
          <button className="send" onClick={() => run('Pause', () => pauseToken(address, id, sign))}>Pause</button>{' '}
          <button className="send" onClick={() => run('Unpause', () => unpauseToken(address, id, sign))}>Unpause</button></>
      )}

      {cfg.denylist && (
        <><h3>Denylist</h3>
          <div className="field"><input placeholder="account G…" value={attAcct} onChange={(e) => setAttAcct(e.target.value)} />
            <button className="send" onClick={() => run('Denylist', () => addToDenylist(address, id, attAcct, sign))}>Add to denylist</button></div></>
      )}

      <h3>Read balance</h3>
      <div className="field"><input placeholder="account G…" value={mintTo} onChange={(e) => setMintTo(e.target.value)} />
        <button className="send" onClick={async () => setBal(await readTokenBalance(id, mintTo))}>Read</button>{bal && <span className="pill">{bal}</span>}</div>

      {msg && <div className="result">{msg}</div>}
      {err && <div className="result denied">{err}</div>}
    </section>
  );
}
