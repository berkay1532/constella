import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useWallet } from '../wallet';
import { getToken } from '../tokenStore';
import {
  mint, transfer, attestCountry, readIdentity, readInvestorCount, readTokenBalance, readIsDenied,
  setInvestorCap, setMaxBalance, setMaxHolders, pauseToken, unpauseToken, addToDenylist,
  proveEligibility, readIsVerified,
} from '../hub';

const EXPLORER = 'https://stellar.expert/explorer/testnet';

const ZK_PROVE_STEPS = [
  'Generating witness & Groth16 proof in your browser',
  'Encoding proof for the on-chain verifier (BLS12-381)',
  'Registering your private commitment',
  'Verifying the proof on-chain',
];

// Turn a raw Soroban HostError (with its verbose diagnostic-event log) into a plain reason.
// The diagnostic events name the module hook that returned false, so we key off those.
function humanize(msg: string): string {
  if (/is_verified/.test(msg)) return "a party isn't ZK-eligible yet — prove eligibility in the console first (a mint checks the recipient; a transfer checks both the sender and the recipient).";
  if (/country_of/.test(msg)) return "a party's country isn't attested or isn't in the allowed list (a mint checks the recipient; a transfer checks both sender and recipient).";
  if (/is_denied|Denied|denylist/i.test(msg)) return 'recipient is on the denylist.';
  if (/is_paused|paused|window/i.test(msg)) return 'transfers are currently frozen (transfer window).';
  if (/Error\(Contract, #6\)|can_transfer|can_create/.test(msg)) return "rejected by a compliance rule — the recipient isn't eligible (not attested / not proven).";
  const m = msg.match(/Error\([^)]+\)/);
  return m ? `on-chain error ${m[0]}` : msg.slice(0, 160);
}

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
  const [xferTo, setXferTo] = useState('');
  const [xferAmt, setXferAmt] = useState('10');
  const [zkCountry, setZkCountry] = useState('840');
  const [zkStep, setZkStep] = useState(-1);
  const [verified, setVerified] = useState(false);
  const [tab, setTab] = useState<'ops' | 'settings'>('ops');

  const cfg = rec?.config;
  useEffect(() => {
    if (cfg && (cfg.country_restrict.length || cfg.max_investors)) readIdentity(id).then(setIdentity).catch(() => {});
    if (cfg && cfg.zk_eligibility && address) readIsVerified(id, address).then(setVerified).catch(() => {});
    if (address) setMintTo((cur) => cur || address); // default: mint to yourself
  }, [id, address]); // eslint-disable-line react-hooks/exhaustive-deps

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
    catch (e) { setErr(`${label} rejected: ${humanize(String((e as Error).message || e))}`); setMsg(''); }
  };
  const read = async (label: string, fn: () => Promise<string>) => {
    setErr('');
    try { setBalLabel(label); setBal(await fn()); } catch (e) { setErr(`${label} failed: ${humanize(String((e as Error).message || e))}`); }
  };
  const proveNow = async () => {
    setErr(''); setMsg(''); setZkStep(0);
    try {
      await proveEligibility(id, address, Number(zkCountry), sign, (p) => setZkStep(p === 'register' ? 2 : 3));
      setZkStep(4); setVerified(true); setMsg('Proven eligible — your country stayed private.');
    } catch (e) {
      setZkStep(-1);
      const detail = String((e as Error).message || e);
      setErr(detail.includes('not in the allowed') ? 'Not eligible — and the app never learned your country.' : `Prove rejected: ${detail}`);
    }
  };

  const chips = [
    cfg.denylist && 'denylist',
    cfg.max_balance !== '0' && 'max-balance',
    cfg.country_restrict.length && (cfg.zk_eligibility ? 'country · ZK private' : 'country'),
    cfg.max_holders && `max-holders · ${cfg.max_holders}`,
    cfg.lockup && `lockup · ${cfg.lockup}s`,
    cfg.transfer_window && 'transfer-window',
    cfg.max_investors && `max-investors · ${cfg.max_investors}`,
  ].filter(Boolean) as string[];

  const hasSettings = cfg.denylist || cfg.max_balance !== '0' || cfg.max_holders > 0
    || cfg.transfer_window || cfg.max_investors > 0
    || (!cfg.zk_eligibility && cfg.country_restrict.length > 0);

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

        <div className="console-tabs">
          <button className={`ctab ${tab === 'ops' ? 'on' : ''}`} onClick={() => setTab('ops')}>Token operations</button>
          {hasSettings && (
            <button className={`ctab ${tab === 'settings' ? 'on' : ''}`} onClick={() => setTab('settings')}>Compliance settings</button>
          )}
        </div>

        {tab === 'ops' && (cfg.zk_eligibility && !verified ? (
          <div className="gate">
            <span className="gate-badge">◆ Step 1 · Prove eligibility</span>
            <h4>This token is ZK-gated — prove your eligibility to unlock minting &amp; transfers.</h4>
            <p>
              Prove your country is in the allowed set <strong>without revealing which country it is</strong> —
              a Groth16 proof generated entirely in your browser. Your country never leaves your device and is
              never written on-chain; only the proof of eligibility is.
            </p>
            <div className="op-row">
              <select className="inp narrow" value={zkCountry} onChange={(e) => setZkCountry(e.target.value)}>
                <option value="840">🇺🇸 US (840)</option>
                <option value="276">🇩🇪 DE (276)</option>
                <option value="792">🇹🇷 TR (792)</option>
                <option value="250">🇫🇷 FR (250)</option>
              </select>
              <button className="btn cyan" disabled={zkStep >= 0 && zkStep < 4} onClick={proveNow}>
                {zkStep >= 0 && zkStep < 4 ? 'Proving…' : 'Prove my eligibility'}
              </button>
            </div>
            {zkStep >= 0 && (
              <ul className="zk-pipe">
                {ZK_PROVE_STEPS.map((s, i) => (
                  <li key={i} className={i < zkStep ? 'done' : i === zkStep ? 'active' : ''}>
                    <span className="zs">{i < zkStep ? '✓' : i === zkStep ? '' : i + 1}</span><span>{s}</span>
                  </li>
                ))}
              </ul>
            )}
            <div className="gate-locked">🔒 Mint, Transfer &amp; Read balance unlock the moment you're eligible.</div>
          </div>
        ) : (
          <>
            {cfg.zk_eligibility && verified && (
              <div className="gate-done">✓ Eligibility proven — your country stayed private. Minting &amp; transfers are unlocked.</div>
            )}
            <div className="ops">
              <div className="op">
                <h4>Mint</h4>
                <p className="op-sub">Seed a holder. The recipient defaults to your wallet; compliance is checked on-chain before the mint settles.</p>
                <div className="op-row">
                  <input className="inp" placeholder="recipient G…" value={mintTo} onChange={(e) => setMintTo(e.target.value)} />
                  <input className="inp narrow" type="number" value={mintAmt} onChange={(e) => setMintAmt(e.target.value)} />
                  <button className="btn sm" onClick={() => run('Mint', () => mint(address, id, mintTo, mintAmt, sign))}>Mint</button>
                </div>
              </div>

              <div className="op">
                <h4>Transfer</h4>
                <p className="op-sub">Send tokens from your wallet to another holder. <code>can_transfer</code> is checked on-chain before it settles.</p>
                <div className="op-row">
                  <input className="inp" placeholder="recipient G…" value={xferTo} onChange={(e) => setXferTo(e.target.value)} />
                  <input className="inp narrow" type="number" value={xferAmt} onChange={(e) => setXferAmt(e.target.value)} />
                  <button className="btn sm" onClick={() => run('Transfer', () => transfer(address, id, xferTo, xferAmt, sign))}>Transfer</button>
                </div>
                {cfg.zk_eligibility && (
                  <div className="info-box zk">
                    <span className="ib-ico">🔒</span>
                    <span>
                      The recipient must <strong>prove their own eligibility first</strong> — from their own wallet, in this
                      console. You can't prove on their behalf: the Groth16 proof needs their private country input and their
                      signature. A transfer checks <strong>both</strong> parties, so an un-proven recipient is rejected on-chain.
                    </span>
                  </div>
                )}
                {!cfg.zk_eligibility && cfg.country_restrict.length > 0 && (
                  <div className="info-box">
                    <span className="ib-ico">🌍</span>
                    <span>
                      The recipient must be <strong>attested to an allowed country first</strong> — do it under the
                      Compliance settings tab. Both parties are checked, so an un-attested recipient is rejected on-chain.
                    </span>
                  </div>
                )}
              </div>

              <div className="op full">
                <h4>Read a balance</h4>
                <p className="op-sub">Query any account's balance on this token.</p>
                <div className="op-row">
                  <input className="inp" placeholder="account G…" value={mintTo} onChange={(e) => setMintTo(e.target.value)} />
                  <button className="btn sm ghost" onClick={() => read('balance', () => readTokenBalance(id, mintTo))}>Read balance</button>
                </div>
              </div>
            </div>
          </>
        ))}

        {tab === 'settings' && hasSettings && (
          <div className="ops">
          {!cfg.zk_eligibility && (cfg.country_restrict.length > 0 || cfg.max_investors > 0) && (
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
          </div>
        )}

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
