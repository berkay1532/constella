import { Link } from 'react-router-dom';

const MODULES = [
  { icon: '⛔', name: 'Denylist', kind: 'pre-check', desc: 'Blocks named accounts from receiving or holding the token. Managed live from the console.' },
  { icon: '🌍', name: 'Country restrict', kind: 'identity', desc: 'Only holders attested to an allowed ISO country can receive tokens. Reads from the token’s own identity provider.' },
  { icon: '⚖️', name: 'Max balance', kind: 'stateful', desc: 'Caps the balance any single holder may accumulate. Enforced on every mint and transfer.' },
  { icon: '👥', name: 'Max holders', kind: 'stateful', desc: 'Caps the total number of distinct holders. New holders are rejected once the cap is reached.' },
  { icon: '⏳', name: 'Lockup', kind: 'stateful', desc: 'Locks a holder’s tokens for a fixed number of seconds after they acquire them.' },
  { icon: '🕒', name: 'Transfer window', kind: 'config', desc: 'Freeze all transfers, or open them only within a scheduled window. Toggled by the issuer.' },
  { icon: '🪐', name: 'Max investors / country', kind: 'stateful · identity', desc: 'Caps the number of distinct holders attributed to any single country — combines the balance mirror with per-country identity.' },
];

export function Docs() {
  return (
    <section className="block">
      <div className="sec-head">
        <span className="eyebrow">Documentation</span>
        <h2>How Constella works.</h2>
        <p>A no-code way to launch and operate compliant real-world-asset tokens on Stellar — every rule enforced on-chain.</p>
      </div>

      <div className="docs">
        <nav className="docs-nav">
          <a href="#overview">Overview</a>
          <a href="#standards">Standards &amp; SEP-57</a>
          <a href="#launch">Launching</a>
          <a href="#modules">Modules</a>
          <a href="#identity">Identity</a>
          <a href="#privacy">ZK privacy</a>
          <a href="#network">Network</a>
        </nav>

        <div className="docs-body">
          <h3 id="overview">Overview</h3>
          <p>
            Constella lets an issuer launch a real, restricted token whose compliance rules live in
            Soroban smart contracts. You choose which rules apply, and they are enforced by the chain
            on every mint and transfer — not by a server you have to trust. The token you deploy is
            yours: your wallet is its sole admin.
          </p>
          <p>
            Every token is served by one shared multi-tenant <em>hub</em>. Launching does not deploy a
            new stack per issuer — it registers your token against the shared modules and configures
            them for you, which is why a full compliant token launches in a single signature.
          </p>

          <h3 id="standards">Standards &amp; SEP-57</h3>
          <p>
            Every regulated tokenized asset has to answer two questions on <em>every</em> transfer:
            <strong> who is allowed to hold it</strong> (identity — jurisdiction, KYC, accreditation) and
            <strong> which rules apply</strong> (compliance — holder caps, lock-ups, concentration limits). On
            Stellar, this two-layer model is being standardized as{' '}
            <a href="https://github.com/orgs/stellar/discussions/1814" target="_blank" rel="noreferrer">SEP-57</a>.
          </p>
          <p>
            SEP-57 is a draft Stellar Ecosystem Proposal for permissioned real-world-asset tokens, led by
            OpenZeppelin. It describes a hook-based compliance engine (modules run <code>created</code> /{' '}
            <code>transferred</code> / <code>destroyed</code> hooks inside the transfer), an abstract identity
            interface that can be claim-based, Merkle-tree, or <strong>zero-knowledge</strong>, and a{' '}
            <a href="https://developers.stellar.org/docs/tokens/token-interface" target="_blank" rel="noreferrer">SEP-41</a>{' '}
            token extended with regulatory controls.
          </p>
          <p>
            Constella follows this architecture directly: a hook-based modular compliance dispatcher, a per-token
            identity provider, an optional zero-knowledge identity that proves eligibility without revealing the
            underlying attribute, and shared multi-tenant infrastructure so many tokens reuse one audited stack. In
            other words, Constella is a working implementation of the identity-and-compliance model SEP-57
            describes — what you launch here maps onto the standard the Stellar ecosystem is converging on. SEP-57
            is an evolving draft, and we track it as its interfaces firm up.
          </p>

          <h3 id="launch">Launching a token</h3>
          <p>
            In the <Link to="/launch">launch wizard</Link>, connect your wallet, switch on the modules
            your asset needs, and review. Pressing <strong>Launch</strong> sends one transaction that
            deploys your token, wires the selected modules, and — when a rule needs it — deploys a
            per-token identity provider. Nothing is minted until you mint it.
          </p>
          <p>
            After launch you land in the token console, and the token appears under
            <Link to="/tokens"> My tokens</Link> for this browser. From the console you mint holders,
            attest identities, adjust caps, and watch rejected actions surface their reason.
          </p>

          <h3 id="modules">Compliance modules</h3>
          <p>Seven modules ship today. Enable any combination; each is enforced independently, per token.</p>
          <ul className="doc-mods">
            {MODULES.map((m) => (
              <li key={m.name}>
                <div className="dm-star">{m.icon}</div>
                <div>
                  <div className="dm-name">{m.name}<span>{m.kind}</span></div>
                  <div className="dm-desc">{m.desc}</div>
                </div>
              </li>
            ))}
          </ul>

          <h3 id="identity">Identity &amp; attestation</h3>
          <p>
            Country-based rules read from an identity provider deployed for your token at launch. Since
            there is no licensed KYC issuer in this deployment, <strong>you are the attestor</strong>:
            from the console you write each holder’s country to your token’s identity. A token that uses
            both country restrict and max-investors shares a single identity, so an attestation counts
            for both.
          </p>

          <h3 id="privacy">Zero-knowledge eligibility</h3>
          <p>
            Country eligibility can be enforced <em>privately</em>. At launch, turn on
            <strong> “Private (prove with ZK)”</strong> under Country eligibility: the hub deploys a
            per-token ZK identity for your token, and holders prove — in the token console — that their
            country is in the allowed set <em>without revealing which country it is</em>, using a Groth16
            proof generated entirely in the browser. The country value never leaves the device and is
            never written on-chain; only the proof of eligibility is. A mint to an un-proven recipient is
            rejected on-chain; a <strong>transfer requires both the sender and the recipient</strong> to
            have proven eligibility — just like every other rule, enforced by the hub.
          </p>
          <p>
            The circuit is fixed to two allowed countries, and private eligibility replaces cleartext
            country attestation for that token (it can’t be combined with the per-country investor cap,
            which needs to read a country).
          </p>

          <h3 id="network">Network</h3>
          <p>
            Constella runs on Stellar <code>testnet</code>. Connect Freighter set to Testnet; launches
            and console actions are signed by your wallet and submitted directly to the network. Token
            and hub addresses link out to <a href="https://stellar.expert/explorer/testnet" target="_blank" rel="noreferrer">stellar.expert</a>.
          </p>
        </div>
      </div>
    </section>
  );
}
