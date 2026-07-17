import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useWallet } from '../wallet';
import { getToken } from '../tokenStore';
import {
  mint, attestCountry, readIdentity, readInvestorCount, readTokenBalance, readIsDenied,
  setInvestorCap, setMaxBalance, setMaxHolders, pauseToken, unpauseToken, addToDenylist,
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
  const [balLabel, setBalLabel] = useState('');
  const [mbCap, setMbCap] = useState('0');
  const [mhCap, setMhCap] = useState(0);

  const cfg = rec?.config;
  useEffect(() => {
    if (cfg && (cfg.country_restrict.length || cfg.max_investors)) readIdentity(id).then(setIdentity).catch(() => {});
  }, [id]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!address) {
    return <div className="panel state"><h2>Token console</h2><p>Connect your wallet to operate a token you launched.</p></div>;
  }
  if (!rec || !cfg) {
    return (
      <div className="panel state">
        <h2>Token not found</h2>
        <p>This browser has no record of that token. Launch one to get started.</p>
        <Link to="/launch" className="btn">Open the launch wizard →</Link>
      </div>
    );
  }

  const run = async (label: string, fn: () => Promise<string | void>) => {
    setMsg(`${label}…`); setErr('');
    try { const h = await fn(); setMsg(`${label} succeeded${typeof h === 'string' ? ` · ${h.slice(0, 10)}…` : ''}`); }
    catch (e) { setErr(`${label} rejected: ${String((e as Error).message || e)}`); setMsg(''); }
  };
  const read = async (label: string, fn: () => Promise<string>) => {
    setErr('');
    try { setBalLabel(label); setBal(await fn()); } catch (e) { setErr(`${label} failed: ${String((e as Error).message || e)}`); }
  };

  const chips = [
    cfg.denylist && 'denylist',
    cfg.max_balance !== '0' && 'max-balance',
    cfg.country_restrict.length && 'country',
    cfg.max_holders && `max-holders · ${cfg.max_holders}`,
    cfg.lockup && `lockup · ${cfg.lockup}s`,
    cfg.transfer_window && 'transfer-window',
    cfg.max_investors && `max-investors · ${cfg.max_investors}`,
  ].filter(Boolean) as string[];

  return (
    <section className="block">
      <div className="sec-head">
        <span className="eyebrow">Token console</span>
        <h2>Operate your token — and watch the rules reject in real time.</h2>
        <p>Mint holders, attest their country, manage caps. When an action breaks a rule, the hub rejects it at simulation — before any signature — and tells you which module said no.</p>
      </div>

      <div className="panel">
        <div className="tok-head">
          <div className="tok-id">
            <span className="label">Token</span>
            <a className="addr" href={`${EXPLORER}/contract/${id}`} target="_blank" rel="noreferrer">{id.slice(0, 8)}…{id.slice(-6)}</a>
          </div>
          <div className="tok-chips">
            {chips.length ? chips.map((c) => <span key={c} className="tag">{c}</span>) : <span className="tag">no modules</span>}
          </div>
        </div>

        <div className="ops">
          <div className="op">
            <h4>Mint</h4>
            <p className="op-sub">Seed a holder. Compliance is checked on-chain before the mint settles.</p>
            <div className="op-row">
              <input className="inp" placeholder="recipient G…" value={mintTo} onChange={(e) => setMintTo(e.target.value)} />
              <input className="inp narrow" type="number" value={mintAmt} onChange={(e) => setMintAmt(e.target.value)} />
              <button className="btn sm" onClick={() => run('Mint', () => mint(address, id, mintTo, mintAmt, sign))}>Mint</button>
            </div>
          </div>

          {(cfg.country_restrict.length > 0 || cfg.max_investors > 0) && (
            <div className="op">
              <h4>Attest identity</h4>
              <p className="op-sub">You are the attestor — write a holder's country to this token's identity{identity ? ` · ${identity.slice(0, 8)}…` : ''}.</p>
              <div className="op-row">
                <input className="inp" placeholder="account G…" value={attAcct} onChange={(e) => setAttAcct(e.target.value)} />
                <input className="inp narrow" value={attCode} onChange={(e) => setAttCode(e.target.value)} placeholder="ISO e.g. 840" />
                <button className="btn sm ghost" onClick={() => run('Attest', () => attestCountry(address, id, attAcct, Number(attCode), sign))}>Attest</button>
              </div>
            </div>
          )}

          {cfg.max_investors > 0 && (
            <div className="op">
              <h4>Max investors / country</h4>
              <p className="op-sub">Cap distinct holders per jurisdiction. Read the live count or raise the cap.</p>
              <div className="op-row">
                <button className="btn sm ghost" onClick={() => run('Raise cap to 2', () => setInvestorCap(address, id, 2, sign))}>Raise cap → 2</button>
                <button className="btn sm ghost" onClick={() => read(`count(${attCode})`, () => readInvestorCount(id, Number(attCode || 840)))}>Read count</button>
              </div>
            </div>
          )}

          {cfg.transfer_window && (
            <div className="op">
              <h4>Transfer window</h4>
              <p className="op-sub">Freeze or unfreeze all transfers for this token.</p>
              <div className="op-row">
                <button className="btn sm ghost" onClick={() => run('Pause', () => pauseToken(address, id, sign))}>Pause</button>
                <button className="btn sm ghost" onClick={() => run('Unpause', () => unpauseToken(address, id, sign))}>Unpause</button>
              </div>
            </div>
          )}

          {cfg.denylist && (
            <div className="op">
              <h4>Denylist</h4>
              <p className="op-sub">Block an account, or check its current status.</p>
              <div className="op-row">
                <input className="inp" placeholder="account G…" value={attAcct} onChange={(e) => setAttAcct(e.target.value)} />
                <button className="btn sm danger" onClick={() => run('Denylist', () => addToDenylist(address, id, attAcct, sign))}>Deny</button>
                <button className="btn sm ghost" onClick={() => read('is_denied', async () => String(await readIsDenied(id, attAcct)))}>Check</button>
              </div>
            </div>
          )}

          {cfg.max_balance !== '0' && (
            <div className="op">
              <h4>Max balance</h4>
              <p className="op-sub">Change the per-holder balance cap after launch.</p>
              <div className="op-row">
                <input className="inp" value={mbCap} onChange={(e) => setMbCap(e.target.value)} placeholder="new per-holder cap" />
                <button className="btn sm ghost" onClick={() => run('Set max balance', () => setMaxBalance(address, id, mbCap, sign))}>Set</button>
              </div>
            </div>
          )}

          {cfg.max_holders > 0 && (
            <div className="op">
              <h4>Max holders</h4>
              <p className="op-sub">Change the holder-count cap after launch.</p>
              <div className="op-row">
                <input className="inp" type="number" value={mhCap} onChange={(e) => setMhCap(Number(e.target.value))} placeholder="new holder cap" />
                <button className="btn sm ghost" onClick={() => run('Set max holders', () => setMaxHolders(address, id, mhCap, sign))}>Set</button>
              </div>
            </div>
          )}

          <div className="op full">
            <h4>Read a balance</h4>
            <p className="op-sub">Query any account's balance on this token.</p>
            <div className="op-row">
              <input className="inp" placeholder="account G…" value={mintTo} onChange={(e) => setMintTo(e.target.value)} />
              <button className="btn sm ghost" onClick={() => read('balance', () => readTokenBalance(id, mintTo))}>Read balance</button>
            </div>
          </div>
        </div>

        {(bal !== '' || msg || err) && (
          <div className="stat-strip">
            {bal !== '' && <div className="stat"><div className="k">{balLabel || 'value'}</div><div className="v good">{bal}</div></div>}
            {(msg || err) && (
              <div style={{ flex: 1 }}>
                {msg && <div className="result ok">✓ <span>{msg}</span></div>}
                {err && <div className="result denied">⛔ <span>{err}</span></div>}
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
