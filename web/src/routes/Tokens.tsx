import { Link } from 'react-router-dom';
import { useWallet } from '../wallet';
import { listTokens, type TokenRecord } from '../tokenStore';
import type { LaunchConfig } from '../hub';

function chips(cfg: LaunchConfig): string[] {
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

function when(ms: number): string {
  try { return new Date(ms).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); }
  catch { return ''; }
}

export function Tokens() {
  const { address, connect, busy } = useWallet();

  if (!address) {
    return (
      <div className="panel state">
        <h2>Your tokens</h2>
        <p>Connect your wallet to see the compliance tokens you've launched.</p>
        <button className="btn" onClick={connect} disabled={busy}>Connect Freighter</button>
      </div>
    );
  }

  const mine: TokenRecord[] = listTokens(address);

  if (mine.length === 0) {
    return (
      <div className="panel state">
        <h2>No tokens yet</h2>
        <p>You haven't launched a compliance token from this browser. It takes one signature.</p>
        <Link to="/launch" className="btn">Launch your first token →</Link>
      </div>
    );
  }

  return (
    <section className="block">
      <div className="sec-head">
        <span className="eyebrow">My tokens</span>
        <h2>Tokens you've launched.</h2>
        <p>Each token is administered only by this wallet. Open one to mint, attest holders, and manage its rules.</p>
      </div>
      <div className="tok-grid">
        {mine.map((t) => (
          <Link key={t.id} to={`/token/${t.id}`} className="tok-card">
            <div className="tc-addr">{t.id.slice(0, 10)}…{t.id.slice(-6)}</div>
            <div className="tc-chips">
              {chips(t.config).length
                ? chips(t.config).map((c) => <span key={c} className="tag">{c}</span>)
                : <span className="tag">no modules</span>}
            </div>
            <div className="tc-meta">launched {when(t.createdAt)}</div>
            <div className="tc-go">Open console →</div>
          </Link>
        ))}
      </div>
    </section>
  );
}
