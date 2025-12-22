# rootsmith

## Running Tests

### Sparse Merkle Accumulator Test with JSON Data

To run the `test_accumulator_create_proof` test that loads and processes data from `sample-data.json`:

```bash
cargo test test_accumulator_create_proof -- --nocapture
```

This test:

- Loads supply chain data from `sample-data.json`
- Parses and displays all 29 records (batches, products, activity logs, etc.)
- Creates proofs using the Sparse Merkle Accumulator

To run all tests in `sma_prover_verifier`:

```bash
cargo test --test sma_prover_verifier -- --nocapture
```

To run test in `sma_prover_verifier` with fn `test_accumulator_create_proof`
```bash
cargo test test_accumulator_create_proof --test sma_prover_verifier -- --nocapture
```
