import { Link } from 'react-router-dom';
export function Landing() {
  return (
    <section className="card hero">
      <h1>Launch your own compliance token</h1>
      <p>Pick from seven on-chain compliance modules and deploy a real, restricted token on Stellar testnet — in one signature. No code.</p>
      <Link to="/launch" className="send">Launch a token →</Link>
      <p className="muted">Curious about the privacy tech? See the <Link to="/zk">zero-knowledge eligibility demo</Link>.</p>
    </section>
  );
}
