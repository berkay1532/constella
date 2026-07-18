import { useEffect, useReducer, useState } from 'react';
import { Link } from 'react-router-dom';
import { useWallet } from '../wallet';
import { launchToken, blankConfig, type LaunchConfig } from '../hub';
import { saveToken } from '../tokenStore';
import { Constellation } from '../sky';

const EXPLORER = 'https://stellar.expert/explorer/testnet';

function configChips(cfg: LaunchConfig): string[] {
  return [
    cfg.denylist && 'denylist',
    cfg.max_balance !== '0' && 'max-balance',
    cfg.country_restrict.length && 'country',
    cfg.max_holders && `max-holders · ${cfg.max_holders}`,
    cfg.lockup && `lockup · ${cfg.lockup}s`,
    cfg.transfer_window && 'transfer-window',
    cfg.max_investors && `max-investors · ${cfg.max_investors}`,
  ].filter(Boolean) as string[];
}

type Action = { field: keyof LaunchConfig; value: LaunchConfig[keyof LaunchConfig] };
const reducer = (s: LaunchConfig, a: Action): LaunchConfig => ({ ...s, [a.field]: a.value });

const COUNTRIES = [
  { code: 840, name: 'United States' }, { code: 276, name: 'Germany' },
  { code: 792, name: 'Turkey' }, { code: 250, name: 'France' },
];

const STAR_LABELS: Record<string, string> = {
  denylist: 'Denylist', country: 'Country', maxbal: 'Max bal',
  holders: 'Holders', lockup: 'Lockup', window: 'Window', investors: 'Investors',
};

function activeMods(cfg: LaunchConfig): string[] {
  const a: string[] = [];
  if (cfg.denylist) a.push('denylist');
  if (cfg.country_restrict.length) a.push('country');
  if (cfg.max_balance !== '0' && cfg.max_balance !== '') a.push('maxbal');
  if (cfg.max_holders > 0) a.push('holders');
  if (cfg.lockup > 0) a.push('lockup');
  if (cfg.transfer_window) a.push('window');
  if (cfg.max_investors > 0) a.push('investors');
  return a;
}

export function LaunchWizard() {
  const { address, connect, sign, busy } = useWallet();
  const [cfg, dispatch] = useReducer(reducer, blankConfig(address || ''));
  const [step, setStep] = useState(0);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const [result, setResult] = useState<{ token: string; hash: string } | null>(null);
  const set = (field: keyof LaunchConfig, value: LaunchConfig[keyof LaunchConfig]) => dispatch({ field, value });

  useEffect(() => { if (address && cfg.admin !== address) set('admin', address); }, [address]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!address) {
    return (
      <div className="panel state">
        <h2>Connect to launch</h2>
        <p>Your wallet becomes the token's issuer and admin — the sole authority over its compliance rules.</p>
        <button className="btn" onClick={connect} disabled={busy}>Connect Freighter</button>
      </div>
    );
  }

  const active = activeMods(cfg);
  const toggleCountry = (code: number) => set('country_restrict',
    cfg.country_restrict.includes(code) ? cfg.country_restrict.filter((c) => c !== code) : [...cfg.country_restrict, code]);

  const onLaunch = async () => {
    setError(''); setStatus('Preparing transaction…');
    try {
      setStatus('Awaiting your signature…');
      const { token, hash } = await launchToken(cfg, sign);
      saveToken({ id: token, admin: address, config: cfg, hash, createdAt: Date.now() });
      setStatus('');
      setResult({ token, hash });
    } catch (e) { setError(String((e as Error).message || e)); setStatus(''); }
  };

  if (result) {
    const chips = configChips(cfg);
    return (
      <section className="block">
        <div className="panel launched">
          <div className="launched-constel"><Constellation mods={active} labels={STAR_LABELS} /></div>
          <span className="eyebrow">Launched</span>
          <h2>Your token is live.</h2>
          <p className="muted">A generic compliant token, deployed and wired to {active.length} module{active.length === 1 ? '' : 's'} — administered only by your wallet.</p>
          <div className="launched-rows">
            <div className="lr"><span className="lr-k">Token</span>
              <a className="lr-v" href={`${EXPLORER}/contract/${result.token}`} target="_blank" rel="noreferrer">{result.token.slice(0, 10)}…{result.token.slice(-6)} ↗</a></div>
            <div className="lr"><span className="lr-k">Launch tx</span>
              <a className="lr-v" href={`${EXPLORER}/tx/${result.hash}`} target="_blank" rel="noreferrer">{result.hash.slice(0, 12)}… ↗</a></div>
            <div className="lr"><span className="lr-k">Modules</span>
              <span className="tok-chips">{chips.length ? chips.map((c) => <span key={c} className="tag">{c}</span>) : <span className="tag">none</span>}</span></div>
          </div>
          <div className="launched-cta">
            <Link className="btn" to={`/token/${result.token}`}>Open token console →</Link>
            <button className="btn ghost" onClick={() => { setResult(null); setStep(0); }}>Launch another</button>
          </div>
        </div>
      </section>
    );
  }

  const short = `${address.slice(0, 6)}…${address.slice(-4)}`;
  const launching = !!status && !error;

  const steps = (
    <div className="steps">
      <div className={`step ${step > 0 ? 'done' : step === 0 ? 'active' : ''}`}>1 · Basics</div>
      <div className={`step ${step > 1 ? 'done' : step === 1 ? 'active' : ''}`}>2 · Compliance</div>
      <div className={`step ${step === 2 ? 'active' : ''}`}>3 · Review</div>
    </div>
  );

  return (
    <section className="block">
      <div className="sec-head">
        <span className="eyebrow">Launch wizard</span>
        <h2>Every rule you switch on lights up in your token's constellation.</h2>
        <p>Each module wires into the shared on-chain hub for your token. One signature deploys the whole thing.</p>
      </div>

      <div className="panel">
        {step === 0 && (
          <div className="wiz-left" style={{ borderRight: 'none' }}>
            {steps}
            <h3 style={{ margin: '0 0 6px' }}>Token basics</h3>
            <p>Issuer / admin: <span className="pill">{short}</span></p>
            <p className="muted">A generic compliant token is deployed under your control. You choose its restrictions next — nothing is minted until you say so.</p>
            <div className="wiz-foot">
              <span className="summary">generic token · admin {short}</span>
              <button className="btn" onClick={() => setStep(1)}>Choose compliance →</button>
            </div>
          </div>
        )}

        {step === 1 && (
          <div className="wiz">
            <div className="wiz-left">
              {steps}
              <div className={`mod ${cfg.denylist ? 'on' : ''}`} role="button" tabIndex={0}
                onClick={() => set('denylist', !cfg.denylist)}
                onKeyDown={(e) => { if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); set('denylist', !cfg.denylist); } }}>
                <div className="star">⛔</div>
                <div><div className="m-name">Denylist</div><div className="m-desc">Block specific accounts from holding</div></div>
                <div className="m-ctl"><div className="sw" /></div>
              </div>

              <div className={`mod ${cfg.country_restrict.length ? 'on' : ''}`}>
                <div className="star">🌍</div>
                <div style={{ flex: 1 }}>
                  <div className="m-name">Country restrict</div>
                  <div className="m-desc">Allow only chosen jurisdictions</div>
                  <div className="countries">
                    {COUNTRIES.map((c) => (
                      <label key={c.code}>
                        <input type="checkbox" checked={cfg.country_restrict.includes(c.code)} onChange={() => toggleCountry(c.code)} />
                        {c.name}
                      </label>
                    ))}
                  </div>
                </div>
              </div>

              <div className={`mod ${cfg.max_balance !== '0' && cfg.max_balance !== '' ? 'on' : ''}`}>
                <div className="star">⚖️</div>
                <div><div className="m-name">Max balance</div><div className="m-desc">Cap tokens per holder (0 = off)</div></div>
                <div className="m-ctl"><input className="num" type="number" min={0} value={cfg.max_balance}
                  onChange={(e) => set('max_balance', e.target.value || '0')} /></div>
              </div>

              <div className={`mod ${cfg.max_holders > 0 ? 'on' : ''}`}>
                <div className="star">👥</div>
                <div><div className="m-name">Max holders</div><div className="m-desc">Cap the number of holders (0 = off)</div></div>
                <div className="m-ctl"><input className="num" type="number" min={0} value={cfg.max_holders}
                  onChange={(e) => set('max_holders', Number(e.target.value))} /></div>
              </div>

              <div className={`mod ${cfg.lockup > 0 ? 'on' : ''}`}>
                <div className="star">⏳</div>
                <div><div className="m-name">Lockup</div><div className="m-desc">Time-lock in seconds after acquisition (0 = off)</div></div>
                <div className="m-ctl"><input className="num" type="number" min={0} value={cfg.lockup}
                  onChange={(e) => set('lockup', Number(e.target.value))} /></div>
              </div>

              <div className={`mod ${cfg.transfer_window ? 'on' : ''}`} role="button" tabIndex={0}
                onClick={() => set('transfer_window', !cfg.transfer_window)}
                onKeyDown={(e) => { if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); set('transfer_window', !cfg.transfer_window); } }}>
                <div className="star">🕒</div>
                <div><div className="m-name">Transfer window</div><div className="m-desc">Start paused / schedule transfers</div></div>
                <div className="m-ctl"><div className="sw" /></div>
              </div>

              <div className={`mod ${cfg.max_investors > 0 ? 'on' : ''}`}>
                <div className="star">🪐</div>
                <div><div className="m-name">Max investors / country</div><div className="m-desc">Cap distinct holders per jurisdiction (0 = off)</div></div>
                <div className="m-ctl"><input className="num" type="number" min={0} value={cfg.max_investors}
                  onChange={(e) => set('max_investors', Number(e.target.value))} /></div>
              </div>

              {cfg.country_restrict.length > 0 && cfg.max_investors > 0 && (
                <div className="wiz-hint">✦ Country restrict and max-investors will share one identity for this token.</div>
              )}

              <div className="wiz-foot">
                <button className="btn ghost sm" onClick={() => setStep(0)}>← Back</button>
                <button className="btn" onClick={() => setStep(2)}>Review →</button>
              </div>
            </div>

            <div className="wiz-right">
              <div className="constel-head">
                <span className="eyebrow">Your token</span>
                <span className="count">{active.length} {active.length === 1 ? 'module wired' : 'modules wired'}</span>
              </div>
              <Constellation mods={active} labels={STAR_LABELS} />
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="wiz-left" style={{ borderRight: 'none' }}>
            {steps}
            <h3 style={{ margin: '0 0 10px' }}>Review &amp; launch</h3>
            <ul className="review">
              <li><span className="k">Admin</span><span className="v">{short}</span></li>
              <li><span className="k">Denylist</span><span className={`v ${cfg.denylist ? 'on' : 'off'}`}>{cfg.denylist ? 'on' : 'off'}</span></li>
              <li><span className="k">Max balance</span><span className={`v ${cfg.max_balance !== '0' ? 'on' : 'off'}`}>{cfg.max_balance === '0' ? 'off' : cfg.max_balance}</span></li>
              <li><span className="k">Country allow-list</span><span className={`v ${cfg.country_restrict.length ? 'on' : 'off'}`}>{cfg.country_restrict.length ? cfg.country_restrict.join(', ') : 'off'}</span></li>
              <li><span className="k">Max holders</span><span className={`v ${cfg.max_holders ? 'on' : 'off'}`}>{cfg.max_holders || 'off'}</span></li>
              <li><span className="k">Lockup</span><span className={`v ${cfg.lockup ? 'on' : 'off'}`}>{cfg.lockup ? `${cfg.lockup}s` : 'off'}</span></li>
              <li><span className="k">Transfer window</span><span className={`v ${cfg.transfer_window ? 'on' : 'off'}`}>{cfg.transfer_window ? 'on' : 'off'}</span></li>
              <li><span className="k">Max investors / country</span><span className={`v ${cfg.max_investors ? 'on' : 'off'}`}>{cfg.max_investors || 'off'}</span></li>
            </ul>
            <div className="wiz-foot">
              <button className="btn ghost sm" onClick={() => setStep(1)}>← Back</button>
              <button className="btn" onClick={onLaunch} disabled={launching}>Launch (one signature)</button>
            </div>
            {status && <div className="result">{status}</div>}
            {error && <div className="result denied">{error}</div>}
          </div>
        )}
      </div>
    </section>
  );
}
