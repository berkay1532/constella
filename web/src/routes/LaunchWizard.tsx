import { useReducer, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useWallet } from '../wallet';
import { launchToken, blankConfig, type LaunchConfig } from '../hub';
import { saveToken } from '../tokenStore';

type Action = { field: keyof LaunchConfig; value: LaunchConfig[keyof LaunchConfig] };
const reducer = (s: LaunchConfig, a: Action): LaunchConfig => ({ ...s, [a.field]: a.value });

const COUNTRIES = [ { code: 840, name: 'United States' }, { code: 276, name: 'Germany' },
  { code: 792, name: 'Turkey' }, { code: 250, name: 'France' } ];

export function LaunchWizard() {
  const { address, connect, sign, busy } = useWallet();
  const [cfg, dispatch] = useReducer(reducer, blankConfig(address || ''));
  const [step, setStep] = useState(0);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const nav = useNavigate();
  const set = (field: keyof LaunchConfig, value: LaunchConfig[keyof LaunchConfig]) => dispatch({ field, value });

  if (!address) {
    return <section className="card"><h2>Launch</h2><p>Connect your wallet to begin — it becomes the token's issuer/admin.</p>
      <button className="send" onClick={connect} disabled={busy}>Connect Freighter</button></section>;
  }
  // keep admin in sync with the connected wallet
  if (cfg.admin !== address) set('admin', address);

  const toggleCountry = (code: number) => set('country_restrict',
    cfg.country_restrict.includes(code) ? cfg.country_restrict.filter((c) => c !== code) : [...cfg.country_restrict, code]);

  const onLaunch = async () => {
    setError(''); setStatus('Preparing…');
    try {
      setStatus('Awaiting signature…');
      const { token, hash } = await launchToken(cfg, sign);
      saveToken({ id: token, admin: address, config: cfg, hash, createdAt: Date.now() });
      setStatus('Launched!');
      nav(`/token/${token}`);
    } catch (e) { setError(String((e as Error).message || e)); setStatus(''); }
  };

  return (
    <section className="card">
      <div className="wizard-steps">
        {['Basics', 'Compliance', 'Review'].map((s, i) =>
          <div key={s} className={`step ${i === step ? 'active' : ''}`}>{i + 1}. {s}</div>)}
      </div>

      {step === 0 && (
        <div>
          <h2>Token basics</h2>
          <p>Issuer / admin: <span className="pill">{address.slice(0,6)}…{address.slice(-4)}</span></p>
          <p className="muted">A generic compliant token is deployed under your control. You configure its restrictions next.</p>
          <button className="send" onClick={() => setStep(1)}>Next →</button>
        </div>
      )}

      {step === 1 && (
        <div>
          <h2>Compliance modules</h2>
          <div className="mod-row"><span>Denylist (block specific accounts)</span>
            <input type="checkbox" checked={cfg.denylist} onChange={(e) => set('denylist', e.target.checked)} /></div>
          <div className="mod-row"><span>Max balance per holder</span>
            <input type="number" min={0} value={cfg.max_balance} onChange={(e) => set('max_balance', e.target.value || '0')} style={{width:120}} /></div>
          <div className="mod-row"><span>Max holders</span>
            <input type="number" min={0} value={cfg.max_holders} onChange={(e) => set('max_holders', Number(e.target.value))} style={{width:120}} /></div>
          <div className="mod-row"><span>Lockup (seconds)</span>
            <input type="number" min={0} value={cfg.lockup} onChange={(e) => set('lockup', Number(e.target.value))} style={{width:120}} /></div>
          <div className="mod-row"><span>Transfer window (start paused/windowed)</span>
            <input type="checkbox" checked={cfg.transfer_window} onChange={(e) => set('transfer_window', e.target.checked)} /></div>
          <div className="mod-row"><span>Max investors per country</span>
            <input type="number" min={0} value={cfg.max_investors} onChange={(e) => set('max_investors', Number(e.target.value))} style={{width:120}} /></div>
          <div className="field"><label>Country allow-list (country restrict)</label>
            <div style={{display:'flex',gap:12,flexWrap:'wrap'}}>
              {COUNTRIES.map((c) => <label key={c.code} style={{display:'flex',gap:4,alignItems:'center'}}>
                <input type="checkbox" checked={cfg.country_restrict.includes(c.code)} onChange={() => toggleCountry(c.code)} />{c.name}</label>)}
            </div>
          </div>
          {cfg.country_restrict.length > 0 && cfg.max_investors > 0 &&
            <p className="muted">Country-restrict and max-investors share one identity for this token.</p>}
          <button className="send" onClick={() => setStep(0)}>← Back</button>{' '}
          <button className="send" onClick={() => setStep(2)}>Review →</button>
        </div>
      )}

      {step === 2 && (
        <div>
          <h2>Review &amp; launch</h2>
          <ul>
            <li>Admin: {address}</li>
            <li>Denylist: {cfg.denylist ? 'on' : 'off'}</li>
            <li>Max balance: {cfg.max_balance === '0' ? 'off' : cfg.max_balance}</li>
            <li>Country allow-list: {cfg.country_restrict.length ? cfg.country_restrict.join(', ') : 'off'}</li>
            <li>Max holders: {cfg.max_holders || 'off'}</li>
            <li>Lockup: {cfg.lockup ? `${cfg.lockup}s` : 'off'}</li>
            <li>Transfer window: {cfg.transfer_window ? 'on' : 'off'}</li>
            <li>Max investors/country: {cfg.max_investors || 'off'}</li>
          </ul>
          <button className="send" onClick={() => setStep(1)}>← Back</button>{' '}
          <button className="send" onClick={onLaunch} disabled={!!status && !error}>Launch (one signature)</button>
          {status && <p className="muted">{status}</p>}
          {error && <div className="result denied">{error}</div>}
        </div>
      )}
    </section>
  );
}
