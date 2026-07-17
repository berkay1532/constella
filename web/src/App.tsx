import { Link, Routes, Route } from 'react-router-dom';
import { Landing } from './routes/Landing';
import { LaunchWizard } from './routes/LaunchWizard';
import { TokenConsole } from './routes/TokenConsole';
import { LegacyDemo } from './routes/LegacyDemo';
import { useWallet } from './wallet';

export function App() {
  const { address, connect, busy } = useWallet();
  return (
    <div className="wrap">
      <nav className="topnav">
        <Link to="/" className="brand">✨ Constella</Link>
        <div className="navlinks">
          <Link to="/launch">Launch</Link>
          <Link to="/zk">ZK demo</Link>
          {address
            ? <span className="pill">{address.slice(0, 4)}…{address.slice(-4)}</span>
            : <button className="send" onClick={connect} disabled={busy}>Connect</button>}
        </div>
      </nav>
      <Routes>
        <Route path="/" element={<Landing />} />
        <Route path="/launch" element={<LaunchWizard />} />
        <Route path="/token/:id" element={<TokenConsole />} />
        <Route path="/zk" element={<LegacyDemo />} />
      </Routes>
    </div>
  );
}
