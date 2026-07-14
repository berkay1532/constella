import { groth16 } from 'snarkjs';
import type { SnarkProof } from './encode';

const WASM_URL = '/zk/country_eligibility.wasm';
const ZKEY_URL = '/zk/country_eligibility_final.zkey';
// Must match the on-chain policy set via set_policy.
const ALLOWED = ['840', '276'];

export class IneligibleError extends Error {
  constructor() {
    super('Country is not in the allowed set');
    this.name = 'IneligibleError';
  }
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
    // An unsatisfiable witness (disallowed country) surfaces as an assert error.
    throw new IneligibleError();
  }
  return { proof: result.proof as SnarkProof, commitment: String(result.publicSignals[0]) };
}
