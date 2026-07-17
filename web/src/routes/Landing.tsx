import { Link } from 'react-router-dom';
import { Starfield } from '../sky';

export function Landing() {
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
            <Link to="/zk" className="btn cyan">See the ZK demo</Link>
          </div>
          <div className="trust">
            <span className="chip"><span className="dot" />7 compliance modules</span>
            <span className="chip"><span className="dot" />One-signature launch</span>
            <span className="chip"><span className="dot" />Live on Stellar testnet</span>
          </div>
        </div>
      </header>

      <section className="zk-band">
        <div className="zk-in">
          <div className="zk-lock">🛡️</div>
          <div className="zk-txt">
            <h3>Zero-knowledge eligibility</h3>
            <p>
              Holders prove their country qualifies without ever revealing it — a Groth16 proof
              generated entirely in the browser. Privacy is a feature, not an afterthought.
            </p>
          </div>
          <Link to="/zk" className="btn cyan">Open the ZK demo</Link>
        </div>
      </section>
    </>
  );
}
