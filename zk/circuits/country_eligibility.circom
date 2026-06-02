pragma circom 2.0.0;

include "circomlib/circuits/poseidon.circom";

// Proves, without revealing `country`:
//   1) commitment == Poseidon(country, secret)   (binds to an issuer-registered commitment)
//   2) country is a member of the public `allowed` set (root-of-polynomial, no hash needed)
//
// Public signals (in order): commitment (output), allowed[0..N-1] (inputs).
// Private signals: country, secret.
template CountryEligibility(N) {
    signal input country;       // private
    signal input secret;        // private
    signal input allowed[N];    // public
    signal output commitment;   // public (computed by the circuit, registered by the issuer)

    // (1) commitment binding
    component h = Poseidon(2);
    h.inputs[0] <== country;
    h.inputs[1] <== secret;
    commitment <== h.out;

    // (2) set membership: prod_i (country - allowed[i]) == 0
    signal prods[N];
    prods[0] <== country - allowed[0];
    for (var i = 1; i < N; i++) {
        prods[i] <== prods[i - 1] * (country - allowed[i]);
    }
    prods[N - 1] === 0;
}

component main {public [allowed]} = CountryEligibility(2);
