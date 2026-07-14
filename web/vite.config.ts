import { defineConfig, type PluginOption } from 'vite';
import react from '@vitejs/plugin-react';
import { exec, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readFileSync } from 'node:fs';

const run = promisify(exec);
const runFile = promisify(execFile);

/**
 * Dev-only middleware: bootstraps a freshly-connected wallet so it can act as a
 * verified holder. Funds it (friendbot), attests its country (US), and mints tokens.
 * Uses the local `stellar` CLI admin identity (`deployer`) — the admin secret never
 * touches the frontend bundle. Only active under `vite dev`.
 */
function bootstrapPlugin(): PluginOption {
  return {
    name: 'constella-bootstrap',
    configureServer(server) {
      server.middlewares.use('/api/bootstrap', async (req, res) => {
        res.setHeader('content-type', 'application/json');
        try {
          const url = new URL(req.url ?? '', 'http://localhost');
          const account = url.searchParams.get('addr');
          if (!account || !account.startsWith('G')) throw new Error('valid ?addr=G... required');
          const d = JSON.parse(readFileSync(new URL('./src/deployed.testnet.json', import.meta.url), 'utf8'));
          const net = '--network testnet';
          await run(`curl -s "https://friendbot.stellar.org/?addr=${account}"`).catch(() => {});
          await run(
            `stellar contract invoke --id ${d.contracts.identity} --source deployer ${net} -- set_country --account ${account} --code 840`,
          );
          await run(
            `stellar contract invoke --id ${d.contracts.token} --source deployer ${net} -- mint --to ${account} --amount 1000`,
          );
          res.end(JSON.stringify({ ok: true }));
        } catch (e) {
          res.statusCode = 500;
          res.end(JSON.stringify({ ok: false, error: String((e as Error).message || e) }));
        }
      });

      // Submit a real ZK proof on-chain for the connected wallet: register its commitment
      // and prove eligibility (the country stays private). Uses the local CLI deployer.
      server.middlewares.use('/api/zk-prove', async (req, res) => {
        res.setHeader('content-type', 'application/json');
        try {
          const url = new URL(req.url ?? '', 'http://localhost');
          const account = url.searchParams.get('addr');
          if (!account || !account.startsWith('G')) throw new Error('valid ?addr=G... required');
          const d = JSON.parse(readFileSync(new URL('./src/deployed.testnet.json', import.meta.url), 'utf8'));
          const id = d.zk.identityZk as string;
          const commit = d.zk.commitment as string;
          const proof = JSON.stringify(d.zk.proof);
          const base = ['contract', 'invoke', '--id', id, '--source', 'deployer', '--network', 'testnet'];
          await runFile('stellar', [...base, '--', 'register_commitment', '--account', account, '--commitment', commit]);
          const out = await runFile('stellar', [...base, '--send=yes', '--', 'prove_eligibility', '--account', account, '--commitment', commit, '--proof', proof]);
          const hash = (`${out.stdout}\n${out.stderr}`.match(/tx\/([a-f0-9]{64})/) || [])[1] ?? '';
          res.end(JSON.stringify({ ok: true, hash }));
        } catch (e) {
          res.statusCode = 500;
          res.end(JSON.stringify({ ok: false, error: String((e as Error).message || e) }));
        }
      });

      // Admin mints the ZK-gated token to a (ZK-verified) wallet so it can transfer.
      server.middlewares.use('/api/zk-mint', async (req, res) => {
        res.setHeader('content-type', 'application/json');
        try {
          const url = new URL(req.url ?? '', 'http://localhost');
          const account = url.searchParams.get('addr');
          if (!account || !account.startsWith('G')) throw new Error('valid ?addr=G... required');
          const d = JSON.parse(readFileSync(new URL('./src/deployed.testnet.json', import.meta.url), 'utf8'));
          await runFile('stellar', [
            'contract', 'invoke', '--id', d.zk.zkToken, '--source', 'deployer', '--network', 'testnet',
            '--', 'mint', '--to', account, '--amount', '1000',
          ]);
          res.end(JSON.stringify({ ok: true }));
        } catch (e) {
          res.statusCode = 500;
          res.end(JSON.stringify({ ok: false, error: String((e as Error).message || e) }));
        }
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), bootstrapPlugin()],
  define: { global: 'globalThis' },
  optimizeDeps: { include: ['snarkjs'] },
});
