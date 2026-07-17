// Verify launchConfigScVal: round-trips to the input config and emits sorted ScMap keys.
// Inline copy of the encoder (kept in sync with web/src/hub.ts). Run: node web/scripts/verify-launch-encoder.mjs
import { xdr, nativeToScVal, scValToNative } from '@stellar/stellar-sdk';

const addr = (a) => nativeToScVal(a, { type: 'address' });
const i128 = (n) => nativeToScVal(n, { type: 'i128' });
const u32 = (n) => nativeToScVal(n, { type: 'u32' });
const u64 = (n) => nativeToScVal(n, { type: 'u64' });
const u32vec = (arr) => xdr.ScVal.scvVec(arr.map(u32));

function launchConfigScVal(cfg) {
  const e = (k, v) => new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol(k), val: v });
  return xdr.ScVal.scvMap([
    e('admin', addr(cfg.admin)),
    e('country_restrict', u32vec(cfg.country_restrict)),
    e('denylist', xdr.ScVal.scvBool(cfg.denylist)),
    e('lockup', u64(cfg.lockup)),
    e('max_balance', i128(cfg.max_balance)),
    e('max_holders', u32(cfg.max_holders)),
    e('max_investors', u32(cfg.max_investors)),
    e('transfer_window', xdr.ScVal.scvBool(cfg.transfer_window)),
  ]);
}

const cfg = {
  admin: 'GDXK5YGKCYYQYIEGWQNVTQXN7MK6VDDCA5UV4ZYP7TWWEGTMVSW3VIFC',
  denylist: true, max_balance: '1000', country_restrict: [840, 276],
  max_holders: 5, lockup: '3600', transfer_window: false, max_investors: 2,
};
const sv = launchConfigScVal(cfg);
const keys = sv.map().map((en) => en.key().sym().toString());
const sorted = [...keys].sort();
if (JSON.stringify(keys) !== JSON.stringify(sorted)) {
  console.error('MISMATCH: ScMap keys not sorted\n  got:', keys, '\n  want:', sorted);
  process.exit(1);
}
const back = scValToNative(sv);
// Build a canonicalized (sorted-key) copy so JSON.stringify comparison isn't sensitive to
// property insertion order (cfg's literal order vs. scValToNative's sorted-ScMap order differ).
const norm = (o) => Object.fromEntries(
  Object.keys(o).sort().map((k) => {
    if (k === 'max_balance' || k === 'lockup') return [k, String(o[k])];
    if (k === 'country_restrict') return [k, o[k].map(Number)];
    return [k, o[k]];
  }),
);
if (JSON.stringify(norm(back)) !== JSON.stringify(norm(cfg))) {
  console.error('MISMATCH: round-trip\n  got:', norm(back), '\n  want:', norm(cfg));
  process.exit(1);
}
console.log('✅ launchConfigScVal: keys sorted + round-trips to input config');
