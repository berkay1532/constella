// TypeScript port of tools/zk-encode (arkworks uncompressed BLS12-381 layout).
// Field elements are canonical big-endian (arkworks default); Fr is also big-endian.
// Points are x||y (G1) and x.c0||x.c1||y.c0||y.c1 (G2), matching the Rust encoder field order.

function beBytes(x: bigint, n: number): Uint8Array {
  const out = new Uint8Array(n);
  let v = x;
  for (let i = n - 1; i >= 0; i--) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

const be48 = (dec: string): Uint8Array => beBytes(BigInt(dec), 48);

function concat(...parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

/** G1Affine uncompressed: x (48 BE) || y (48 BE) = 96 bytes. */
export function g1(x: string, y: string): Uint8Array {
  return concat(be48(x), be48(y));
}

/** G2Affine uncompressed: x.c0 || x.c1 || y.c0 || y.c1 (each 48 BE) = 192 bytes. */
export function g2(x0: string, x1: string, y0: string, y1: string): Uint8Array {
  return concat(be48(x0), be48(x1), be48(y0), be48(y1));
}

/** Fr big-endian, 32 bytes. */
export function fr(dec: string): Uint8Array {
  return beBytes(BigInt(dec), 32);
}

export function toHex(u8: Uint8Array): string {
  return Array.from(u8, (b) => b.toString(16).padStart(2, '0')).join('');
}

export interface SnarkProof {
  pi_a: string[];
  pi_b: string[][];
  pi_c: string[];
}

/** Encode a snarkjs proof into the { a, b, c } byte blobs prove_eligibility expects. */
export function encodeProof(p: SnarkProof): { a: Uint8Array; b: Uint8Array; c: Uint8Array } {
  return {
    a: g1(p.pi_a[0], p.pi_a[1]),
    b: g2(p.pi_b[0][1], p.pi_b[0][0], p.pi_b[1][1], p.pi_b[1][0]),
    c: g1(p.pi_c[0], p.pi_c[1]),
  };
}
