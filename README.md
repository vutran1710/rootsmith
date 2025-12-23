# rootsmith

## Running Tests

### Sparse Merkle Accumulator Test with JSON Data

To run the `test_accumulator_create_proof` test that loads and processes data from `tests/data/kv-data.json`:

```bash
cargo test test_accumulator_create_proof -- --nocapture
```

This test:

- Loads key-value data from `tests/data/kv-data.json`
- Parses and displays all 30 records
- Creates proofs using the Sparse Merkle Accumulator
- Verifies proofs using the JSON payload

To run all tests in `sma_prover_verifier`:

```bash
cargo test --test sma_prover_verifier -- --nocapture
```

To run test in `sma_prover_verifier` with fn `test_accumulator_create_proof`

```bash
cargo test test_accumulator_create_proof --test sma_prover_verifier -- --nocapture
```
