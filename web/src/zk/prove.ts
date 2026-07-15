import { groth16 } from 'snarkjs';
import type { SnarkProof } from './encode';

const WASM_URL = '/zk/country_eligibility.wasm';
const ZKEY_URL = '/zk/country_eligibility_final.zkey';
// Must match the on-chain policy set via set_policy.
const ALLOWED = ['840', '276'];

export class IneligibleError extends Error {
  constructor(options?: { cause?: unknown }) {
    super('Country is not in the allowed set');
    this.name = 'IneligibleError';
    // Assigned directly (not via super(msg, opts)) so this compiles under the ES2020
    // lib target, where the Error `cause` option isn't in the type sig; the property is
    // supported at runtime in every browser/Node this ships to.
    if (options?.cause !== undefined) (this as { cause?: unknown }).cause = options.cause;
  }
}

/**
 * True only for a circom unsatisfiable-witness / constraint-assert failure — i.e. the
 * private country is genuinely not in the allowed set. snarkjs (via circom_runtime)
 * surfaces this as an "Assert Failed" / "Error in template ..." / "constraint ... not
 * satisfied" message. Any other failure (a 404/network error fetching the wasm/zkey, a
 * malformed input) must NOT be treated as ineligibility.
 */
function isUnsatisfiableWitness(e: unknown): boolean {
  const msg = (e instanceof Error ? e.message : String(e)).toLowerCase();
  return (
    msg.includes('assert failed') ||
    msg.includes('error in template') ||
    msg.includes('constraint') && msg.includes('not satisfied')
  );
}

/**
 * Generate a Groth16 proof in the browser that the (private) country is in the allowed
 * set. The country and secret never leave this function. Returns the proof plus the
 * commitment (public signal 0). Throws IneligibleError if the country is not allowed
 * (the witness is unsatisfiable).
 */
export async function generateProof(
  country: number,
  secret: bigint,
): Promise<{ proof: SnarkProof; commitment: string }> {
  const input = { country: String(country), secret: secret.toString(), allowed: ALLOWED };
  let result;
  try {
    result = await groth16.fullProve(input, WASM_URL, ZKEY_URL);
  } catch (e) {
    // Only a circom unsatisfiable-witness assert means "disallowed country". Any other
    // failure (404/network fetching wasm/zkey, malformed input) must surface honestly
    // rather than be mislabeled as ineligibility.
    if (isUnsatisfiableWitness(e)) throw new IneligibleError({ cause: e });
    throw e;
  }
  return { proof: result.proof as SnarkProof, commitment: String(result.publicSignals[0]) };
}
