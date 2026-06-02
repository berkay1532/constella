import { defineConfig, type PluginOption } from 'vite';
import react from '@vitejs/plugin-react';
import { exec } from 'node:child_process';
import { promisify } from 'node:util';
import { readFileSync } from 'node:fs';

const run = promisify(exec);

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
    },
  };
}

export default defineConfig({
  plugins: [react(), bootstrapPlugin()],
});
