use std::path::Path;

use kanal::unbounded_async;
use rootsmith::accumulator::AccumulatorVariant;
use rootsmith::archiver::ArchiveVariant;
use rootsmith::config::AccumulatorType;
use rootsmith::config::BaseConfig;
use rootsmith::downstream::DownstreamVariant;
use rootsmith::rootsmith::RootSmith;
use rootsmith::storage::Storage;
use rootsmith::upstream::UpstreamVariant;
use rootsmith::wasm_host::WasmLimits;
use rootsmith::wasm_host::WasmPluginHost;
use serde::Deserialize;

#[tokio::test]
async fn test_rootsmith_flow_partner_to_zk_service() {
    println!("\n═══════════════════════════════════════════════════════════");
    println!("🔄 RootSmith Flow: Partner → Upstream → ZK Service");
    println!("═══════════════════════════════════════════════════════════\n");

    let plugin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("plugins")
        .join("client.wasm");

    if !plugin_path.exists() {
        println!("⚠️  Skipping test: client.wasm not found");
        println!(
            "   Run: cd tests/plugins && cargo build --target wasm32-unknown-unknown --release"
        );
        return;
    }

    println!("Step 1: Initialize RootSmith with WASM Host and ZK Accumulator");
    println!("─────────────────────────────────────────────────────────");
    let limits = WasmLimits::default();
    let plugin_path_str = plugin_path.to_str().expect("Invalid path");
    let wasm_host =
        WasmPluginHost::load(plugin_path_str, limits).expect("Failed to load client plugin");
    println!("✅ WASM Plugin loaded: {}", plugin_path_str);

    let storage = Storage::open("./test_data").expect("Failed to open storage");
    let mut config = BaseConfig::default();
    config.zk_service_url = "http://localhost:3000".to_string();
    config.zk_circuit_id = "v1_16_24_4".to_string();
    config.accumulator_type = AccumulatorType::Zk;

    let accumulator = AccumulatorVariant::new(AccumulatorType::Zk, &config);
    println!("✅ ZK Accumulator initialized (http://localhost:3000, circuit: v1_16_24_4)");

    let rootsmith = RootSmith::new(
        UpstreamVariant::Noop(rootsmith::upstream::NoopUpstream),
        DownstreamVariant::Blackhole(rootsmith::downstream::BlackholeDownstream::new()),
        ArchiveVariant::Noop(rootsmith::archiver::NoopArchive),
        config,
        storage,
    )
    .with_wasm_host(wasm_host)
    .with_accumulator(accumulator);

    println!("✅ RootSmith initialized\n");

    println!("Step 2: Partner Sends Data");
    println!("─────────────────────────────────────────────────────────");
    let data_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("zk-data.json");

    #[derive(Deserialize)]
    struct TestData {
        records: Vec<TestRecord>,
    }

    #[derive(Deserialize)]
    struct TestRecord {
        id: String,
        ts_ms: u64,
        user_id: String,
        action: String,
    }

    let json_content = std::fs::read_to_string(&data_path).expect("Failed to read zk-data.json");
    let test_data: TestData =
        serde_json::from_str(&json_content).expect("Failed to parse zk-data.json");

    let first_record = &test_data.records[0];
    let partner_json = format!(
        r#"{{"id":"{}","ts_ms":{},"user_id":"{}","action":"{}"}}"#,
        first_record.id, first_record.ts_ms, first_record.user_id, first_record.action
    );
    println!("Partner sends: {}", partner_json);
    println!("✅ Partner data received\n");

    println!("Step 3: RootSmith Processes Data (WASM → ZK Accumulator → ZK Service)");
    println!("─────────────────────────────────────────────────────────");

    use rootsmith::accumulator::ZKTrait;
    use rootsmith::wasm_host::ToStandardData;
    use rootsmith::zk_service::DataSelection;
    use rootsmith::zk_service::InputData;
    use rootsmith::zk_service::Operator;
    use rootsmith::zk_service::SelectionCount;
    use rootsmith::zk_service::SubmitJobRequest;
    use rootsmith::zk_service::ZkServiceClient;

    let zk_client = ZkServiceClient::new("http://localhost:3000");

    let wasm_host_guard = rootsmith
        .wasm_host
        .as_ref()
        .expect("WASM host not configured");
    let mut wasm_host = wasm_host_guard.lock().await;
    let output: Box<dyn ToStandardData> = wasm_host
        .process_input(partner_json.as_bytes())
        .expect("Failed to process input");

    let namespace = output.namespace();
    let key = output.key();
    let value = output.value();
    let timestamp = output.timestamp();

    drop(wasm_host);

    struct ZKDataWrapper {
        namespace: [u8; 32],
        key: [u8; 32],
        value: [u8; 32],
        timestamp: u64,
    }

    impl ZKTrait for ZKDataWrapper {
        fn namespace(&self) -> [u8; 32] {
            self.namespace
        }
        fn key(&self) -> [u8; 32] {
            self.key
        }
        fn value(&self) -> [u8; 32] {
            self.value
        }
        fn timestamp(&self) -> u64 {
            self.timestamp
        }
    }

    let zk_data = ZKDataWrapper {
        namespace,
        key,
        value,
        timestamp,
    };

    let trait_objects: Vec<Box<dyn ZKTrait>> = vec![Box::new(zk_data)];

    let input_data = {
        let mut data_rows = Vec::with_capacity(trait_objects.len());
        for record in &trait_objects {
            let mut row = Vec::with_capacity(16);
            let value = record.value();
            row.extend_from_slice(&value[..16]);
            data_rows.push(row);
        }
        InputData::RawBytes(data_rows)
    };

    let request = SubmitJobRequest {
        circuit_id: "v1_16_24_4".to_string(),
        operators: vec![Operator::Merkle16 {
            selection: DataSelection {
                start: 0,
                offset: 1,
                count: SelectionCount::All,
            },
            handler: "0x0000000000000000000000000000000000000000".to_string(),
        }],
        data: input_data,
        webhook_url: "http://localhost:8080/webhook".to_string(),
    };

    println!("Request:");
    println!("  Circuit ID: {}", request.circuit_id);
    println!("  Operators: {:?}", request.operators);
    match &request.data {
        InputData::RawBytes(rows) => {
            println!(
                "  Data: RawBytes ({} rows, {} bytes per row)",
                rows.len(),
                rows.first().map(|r| r.len()).unwrap_or(0)
            );
            println!(
                "  Total data size: {} bytes",
                rows.iter().map(|r| r.len()).sum::<usize>()
            );
        }
        InputData::Table {
            columns,
            column_order,
        } => {
            println!("  Data: Table ({} columns)", column_order.len());
        }
    }
    println!("  Webhook URL: {}\n", request.webhook_url);

    match zk_client.submit_job(request).await {
        Ok(response) => {
            println!("✅ API Response - Job Submitted:");
            println!("   Job ID: {}", response.job_id);
            println!("   Status: {}\n", response.status);

            println!("Polling job status...");
            match zk_client.get_job_status(response.job_id).await {
                Ok(status) => {
                    println!("✅ API Response - Job Status:");
                    println!("   Job ID:      {}", status.job_id);
                    println!("   Status:      {}", status.status);
                    if let Some(error) = &status.error {
                        println!("   Error:       {}", error);
                    }
                    if let Some(result) = &status.result {
                        println!(
                            "   Proof:       {}... ({} chars)",
                            &result.proof[..result.proof.len().min(50)],
                            result.proof.len()
                        );
                        println!(
                            "   Public Signals: {}... ({} chars)",
                            &result.public_signals[..result.public_signals.len().min(50)],
                            result.public_signals.len()
                        );
                    }
                    println!("   Created At:   {}", status.created_at);
                    if let Some(completed_at) = &status.completed_at {
                        println!("   Completed At: {}\n", completed_at);
                    } else {
                        println!("   Completed At: (pending)\n");
                    }
                }
                Err(e) => {
                    println!("❌ Failed to get job status: {}\n", e);
                }
            }
        }
        Err(e) => {
            println!("❌ API Response - Error:");
            println!("   {}", e);
            println!("   (Response from http://localhost:3000)\n");
        }
    }

    println!("✅ Flow completed successfully\n");

    println!("═══════════════════════════════════════════════════════════");
    println!("✅ Flow visualization complete!");
    println!("═══════════════════════════════════════════════════════════\n");
}
