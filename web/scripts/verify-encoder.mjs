// Verify the TS encoder matches the Rust tools/zk-encode output byte-for-byte.
// Keeps an inline copy of the three encode primitives (le48/g1/g2) so it runs under
// plain Node with no TypeScript loader — see the note after this script.
// Run: node web/scripts/verify-encoder.mjs
import { readFileSync } from 'node:fs';

const proof = JSON.parse(readFileSync(new URL('../../zk/data/proof.json', import.meta.url), 'utf8'));
const golden = JSON.parse(readFileSync(new URL('../src/zk/golden.json', import.meta.url), 'utf8'));

// Inline copy of the encoder (kept in sync with web/src/zk/encode.ts) to avoid a TS loader.
const beBytes = (x, n) => { const o = new Uint8Array(n); let v = x; for (let i=n-1;i>=0;i--){o[i]=Number(v&0xffn);v>>=8n;} return o; };
const be48 = (d) => beBytes(BigInt(d), 48);
const cat = (...ps) => { const t = ps.reduce((n,p)=>n+p.length,0); const o=new Uint8Array(t); let f=0; for(const p of ps){o.set(p,f);f+=p.length;} return o; };
const g1 = (x,y) => cat(be48(x), be48(y));
// G2 args are snarkjs-natural order (x0,x1,y0,y1); Fq2 is emitted imaginary-first (c1||c0).
const g2 = (x0,x1,y0,y1) => cat(be48(x1), be48(x0), be48(y1), be48(y0));
const hex = (u8) => Array.from(u8, b => b.toString(16).padStart(2,'0')).join('');

const a = hex(g1(proof.pi_a[0], proof.pi_a[1]));
const b = hex(g2(proof.pi_b[0][0], proof.pi_b[0][1], proof.pi_b[1][0], proof.pi_b[1][1]));
const c = hex(g1(proof.pi_c[0], proof.pi_c[1]));

const ok = a === golden.proof.a && b === golden.proof.b && c === golden.proof.c;
if (!ok) {
  console.error('MISMATCH');
  console.error('a', a === golden.proof.a, '\n  ts:', a, '\n  rs:', golden.proof.a);
  console.error('b', b === golden.proof.b);
  console.error('c', c === golden.proof.c);
  process.exit(1);
}
console.log('✅ TS encoder matches Rust golden (a/b/c byte-for-byte)');
