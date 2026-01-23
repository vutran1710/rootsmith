//! Usage Demo - Complete example from README
//!
//! HOW TO RUN:
//! ===========
//!
//! 1. First, download required artifacts (if not already done):
//!    ```bash
//!    ./scripts/download-artifacts.sh
//!    ```
//!
//! 2. Run this example:
//!    ```bash
//!    cargo run --release --example usage_demo
//!    ```
//!
//! This example demonstrates the complete workflow:
//! - Preparing circuit input from raw bytes (recommended approach)
//! - Generating witness using native circom-witnesscalc
//! - Generating ZK proof using rapidsnark
//! - Verifying the proof
//!
//! Expected output: "Proof valid: true" (takes ~4.3s total)

use multiple_circuit_prover::{
    input::{prepare_circuit_input_from_bytes, Operation, OP_MERKLE16},
    witness::generate_witness,
    generate_proof,
};
use multiple_circuit_verifier::verify_proof;

fn main() -> anyhow::Result<()> {
    println!("Starting ZK proof generation demo...\n");

    // Sample data - using numbers that fit within the circuit field
    // Simulating user balances/amounts
    let data: Vec<Vec<u8>> = vec![
        1000u64.to_be_bytes().to_vec(),
        2000u64.to_be_bytes().to_vec(),
        3000u64.to_be_bytes().to_vec(),
        4000u64.to_be_bytes().to_vec(),
        5000u64.to_be_bytes().to_vec(),
        6000u64.to_be_bytes().to_vec(),
        7000u64.to_be_bytes().to_vec(),
        8000u64.to_be_bytes().to_vec(),
        9000u64.to_be_bytes().to_vec(),
        10000u64.to_be_bytes().to_vec(),
    ];

    println!("Data prepared:");
    println!("  - 10 numeric values (1000, 2000, ... 10000)");
    println!("  - Demonstrating bytes → BigInt conversion\n");

    // Define operations
    let operations = vec![
        Operation {
            opcode: OP_MERKLE16,
            start: 0,
            offset: 1,
            count: 10,
            result: None,
            handler_id: 1,
        }
    ];

    println!("Step 1: Preparing circuit input from bytes (~150ms)...");
    let input = prepare_circuit_input_from_bytes(&data, &operations)?;
    println!("✓ Input prepared");

    println!("Step 2: Generating witness (~756ms)...");
    let witness = generate_witness("artifacts/main.graph.bin", &input)?;
    println!("✓ Witness generated\n");

    println!("Step 3: Generating proof (~3.4s)...");
    let result = generate_proof("artifacts/main.zkey", witness)?;
    println!("✓ Proof generated: {:#?}\n", result);

    println!("Step 4: Verifying proof (~2ms)...");
    let vkey = std::fs::read_to_string("artifacts/vkey.json")?;
    let is_valid = verify_proof(&result.proof, &result.public_signals, &vkey)?;
    println!("✓ Verification complete\n");

    println!("=====================================");
    println!("Proof valid: {}", is_valid);
    println!("=====================================");

    Ok(())
}
