# client-testing

Integration test client for rootsmith. Run on **host** while rootsmith runs in Docker.

1. Starts HTTP server on host port 3001 to receive webhook callbacks
2. Sends test data to rootsmith upstream (localhost:8000)
3. Waits for rootsmith to callback with commitment result

Rootsmith in Docker uses `host.docker.internal` to reach the host.

## Prerequisites

- Python 3 (stdlib only, no pip install needed)

## Usage

1. Start rootsmith (docker compose):
   ```bash
   docker compose -f docker-compose.dev.yml up -d
   ```

2. Run client-testing on host (calls rootsmith via localhost:8000, receives webhook on port 3001):
   ```bash
   python client-testing/main.py
   ```
   Or from inside the folder:
   ```bash
   cd client-testing && python main.py
   ```

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ROOTSMITH_UPSTREAM_URL` | `http://localhost:8000` | Rootsmith upstream URL |
| `ROOTSMITH_HEALTH_URL` | `http://localhost:9000/health` | Rootsmith health check URL |
| `CLIENT_WEBHOOK_PORT` | `3001` | Port for webhook server on host |
