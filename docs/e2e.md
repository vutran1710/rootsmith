# End-to-End Integration Testing

This document describes how to run end-to-end integration tests for the RootSmith application.

## Overview

The E2E integration test (`tests/e2e_integration_test.rs`) sends all records from `tests/data/zk-data.json` to the RootSmith HTTP upstream endpoint and verifies successful processing.

## Prerequisites

1. **RootSmith application running** on port 8000 (default)
2. **ZK Service running** on port 3000 (if testing full flow)
3. **WASM plugin built** at `tests/plugins/client.wasm`

## Building the WASM Plugin

Before running tests, ensure the WASM plugin is built:

```bash
cd tests/plugins && cargo build --target wasm32-unknown-unknown --release && cp target/wasm32-unknown-unknown/release/wasm_plugin_client.wasm client.wasm
```

## Running the Application

### Option 1: Using Cargo (Development)

```bash
# Start RootSmith on port 8000
cargo run -- --wasm-plugin-path tests/plugins/client.wasm
```

The application will:
- Listen on `http://127.0.0.1:8000`
- Accept data on `POST /ingest/raw`
- Process data through WASM host → ZK accumulator → ZK service

### Option 2: Using Docker Compose

```bash
# Build and start services
docker-compose -f docker-compose.dev.yml up --build

# Or run in detached mode
docker-compose -f docker-compose.dev.yml up -d --build
```

This will start:
- RootSmith on port 8000
- ZK Service on port 3000 (placeholder - replace with actual service)

## Running Integration Tests

### Basic Test Execution

```bash
# Run all E2E tests
cargo test --test e2e_integration_test -- --ignored

# Run specific test
cargo test --test e2e_integration_test test_e2e_send_all_zk_data -- --ignored

# Run with output
cargo test --test e2e_integration_test -- --ignored --nocapture
```

### Custom RootSmith URL

Set the `ROOTSMITH_URL` environment variable to test against a different endpoint:

```bash
ROOTSMITH_URL=http://localhost:8000 cargo test --test e2e_integration_test -- --ignored
```

### Test What Gets Sent

The test reads `tests/data/zk-data.json` which contains 16 test records. Each record has:
- `id`: Unique event identifier
- `ts_ms`: Timestamp in milliseconds
- `user_id`: User identifier
- `action`: Action type (click, view, purchase, etc.)

## Test Flow

1. **Load Test Data**: Reads all records from `tests/data/zk-data.json`
2. **Send Records**: POSTs each record as JSON to `POST /ingest/raw`
3. **Verify Responses**: Checks HTTP status codes for success
4. **Report Results**: Displays summary of successful/failed sends

## Expected Output

```
═══════════════════════════════════════════════════════════
🔄 E2E Integration Test: Send All Data from zk-data.json
═══════════════════════════════════════════════════════════

📊 Loaded 16 records from zk-data.json
🌐 Sending data to: http://localhost:8000/ingest/raw
✅ [1] Record event-001 sent successfully (status: 200 OK)
✅ [2] Record event-002 sent successfully (status: 200 OK)
...
✅ [16] Record event-016 sent successfully (status: 200 OK)

═══════════════════════════════════════════════════════════
📈 Test Summary
═══════════════════════════════════════════════════════════
Total records: 16
✅ Successful: 16
❌ Failed: 0
═══════════════════════════════════════════════════════════
```

## Manual Testing

You can also manually send data using `curl`:

```bash
# Single record
curl -X POST http://localhost:8000/ingest/raw \
  -H "Content-Type: application/json" \
  -d '{"id":"event-001","ts_ms":1699123456000,"user_id":"user-001","action":"click"}'

# Health check
curl http://localhost:8000/health
```

## Troubleshooting

### Test Fails: Connection Refused

**Problem**: Cannot connect to RootSmith on port 8000

**Solution**: 
- Ensure RootSmith is running: `cargo run -- --wasm-plugin-path tests/plugins/client.wasm`
- Check if port 8000 is available: `lsof -i :8000`
- Verify the URL: `ROOTSMITH_URL=http://localhost:8000`

### Test Fails: WASM Plugin Not Found

**Problem**: `WASM plugin path is required (--wasm-plugin-path)`

**Solution**:
- Build the WASM plugin (see "Building the WASM Plugin" above)
- Ensure the path is correct: `tests/plugins/client.wasm`

### Test Fails: ZK Service Not Available

**Problem**: ZK accumulator cannot connect to ZK service

**Solution**:
- Start ZK service on port 3000
- Or update `--zk-service-url` to point to running service
- For testing without ZK service, you may need to mock the ZK accumulator

## Docker Development

### Building the Docker Image

```bash
docker build -f docker/Dockerfile.dev -t rootsmith-dev .
```

### Running in Docker

```bash
docker run -p 8000:8000 \
  -v $(pwd)/tests:/app/tests \
  -v $(pwd)/data:/app/data \
  rootsmith-dev
```

### Using Docker Compose

```bash
# Start all services
docker-compose -f docker-compose.dev.yml up

# View logs
docker-compose -f docker-compose.dev.yml logs -f rootsmith

# Stop services
docker-compose -f docker-compose.dev.yml down
```

## Next Steps

- Add more test scenarios (batch sending, error handling)
- Integrate with CI/CD pipeline
- Add performance benchmarks
- Test with different WASM plugins
