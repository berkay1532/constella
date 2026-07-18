import { Link } from 'react-router-dom';
import { Starfield } from '../sky';
import { useWallet } from '../wallet';
import { listTokens } from '../tokenStore';

const FEATURES = [
  { icon: '⛔', name: 'Denylist', desc: 'Block specific accounts from ever holding the token.' },
  { icon: '🌍', name: 'Country restrict', desc: 'Allow only holders attested to chosen jurisdictions.' },
  { icon: '⚖️', name: 'Max balance', desc: 'Cap how many tokens any single holder can hold.' },
  { icon: '👥', name: 'Max holders', desc: 'Cap the total number of distinct holders.' },
  { icon: '⏳', name: 'Lockup', desc: 'Time-lock tokens for a set period after acquisition.' },
  { icon: '🕒', name: 'Transfer window', desc: 'Freeze transfers or schedule an open window.' },
  { icon: '🪐', name: 'Max investors / country', desc: 'Cap distinct holders per jurisdiction.' },
  { icon: '🛡️', name: 'ZK eligibility', desc: 'Holders prove they qualify without revealing their country.' },
];

export function Landing() {
  const { address } = useWallet();
  const mine = address ? listTokens(address) : [];

  return (
    <>
      <header className="hero">
        <Starfield className="sky" />
        <div className="glow" />
        <div className="hero-in">
          <span className="eyebrow">Compliance infrastructure · Stellar</span>
          <h1>Launch a compliant token as a <span className="accent">constellation of rules</span>.</h1>
          <p className="lede">
            Pick the compliance modules your asset needs — denylists, country limits, holder caps,
            lockups, per-country investor caps — and deploy a real, restricted token in a single
            signature. No code, no custodian.
          </p>
          <div className="hero-cta">
            <Link to="/launch" className="btn">Launch a token →</Link>
            <Link to="/docs" className="btn ghost">Read the docs</Link>
          </div>
          <div className="trust">
            <span className="chip"><span className="dot" />7 compliance modules</span>
            <span className="chip"><span className="dot" />One-signature launch</span>
            <span className="chip"><span className="dot" />Live on Stellar testnet</span>
          </div>

          {mine.length > 0 && (
            <div className="mine-banner">
              <span className="mb-txt">You have <b>{mine.length}</b> launched {mine.length === 1 ? 'token' : 'tokens'} in this browser.</span>
              <Link to="/tokens" className="btn sm">Open dashboard →</Link>
            </div>
          )}
        </div>
      </header>

      <section className="features">
        <div className="sec-head">
          <span className="eyebrow">What you can enforce</span>
          <h2>Eight on-chain controls, wired into one shared hub.</h2>
          <p>Each control is a live Soroban contract. Enable the ones your asset needs at launch, then tune or extend them from the token console — every rule enforced on-chain, per token.</p>
        </div>
        <div className="feat-grid">
          {FEATURES.map((f) => (
            <div className="feat" key={f.name}>
              <div className="fi">{f.icon}</div>
              <h4>{f.name}</h4>
              <p>{f.desc}</p>
            </div>
          ))}
        </div>
      </section>
    </>
  );
}
