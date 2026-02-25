#!/usr/bin/env python3
"""
Integration test client for rootsmith. Run on host while rootsmith runs in Docker.

1. Starts HTTP server on host port (default 3001) to receive webhook callbacks
2. Sends test data to rootsmith upstream (localhost:8000)
3. Waits for rootsmith to callback with commitment result

Usage:
    python main.py
    # or from repo root:
    python client-testing/main.py

Environment:
    ROOTSMITH_UPSTREAM_URL  - Upstream URL (default: http://localhost:8000)
    ROOTSMITH_HEALTH_URL   - Health check URL (default: http://localhost:9000/health)
    CLIENT_WEBHOOK_PORT    - Port for webhook server (default: 3001)
"""

import json
import os
import sys
import time
import threading
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.request import Request, urlopen
from urllib.error import URLError, HTTPError

ROOTSMITH_UPSTREAM_URL = os.environ.get("ROOTSMITH_UPSTREAM_URL", "http://localhost:8000")
ROOTSMITH_HEALTH_URL = os.environ.get("ROOTSMITH_HEALTH_URL", "http://localhost:9000/health")
CLIENT_WEBHOOK_PORT = int(os.environ.get("CLIENT_WEBHOOK_PORT", "3001"))

# Shared state for webhook received
webhook_received = threading.Event()
webhook_payload = None


class WebhookHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        global webhook_payload
        if self.path == "/webhook" or self.path == "/webhook/":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)

            try:
                payload = json.loads(body.decode())
                webhook_payload = payload
                print(f"\n{'='*50}")
                print("Webhook received from rootsmith!")
                print(f"  job_id: {payload.get('job_id', 'N/A')}")
                commitment = payload.get("commitment", {})
                if isinstance(commitment, dict):
                    print(f"  item_count: {commitment.get('item_count', 'N/A')}")
                print(f"{'='*50}\n")

                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                response = json.dumps({"accepted": True}).encode()
                self.send_header("Content-Length", len(response))
                self.end_headers()
                self.wfile.write(response)

                webhook_received.set()
            except json.JSONDecodeError as e:
                print(f"Invalid JSON in webhook: {e}")
                self.send_response(400)
                self.end_headers()
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        print(f"[HTTP] {args[0]}")


def wait_for_rootsmith(max_attempts=15, interval=2):
    """Wait for rootsmith to be ready."""
    for attempt in range(1, max_attempts + 1):
        try:
            req = Request(ROOTSMITH_HEALTH_URL, method="GET")
            with urlopen(req, timeout=5) as resp:
                if resp.status == 200:
                    print("Rootsmith is ready")
                    return True
        except (URLError, HTTPError, OSError):
            pass
        if attempt < max_attempts:
            print(f"Waiting for rootsmith (attempt {attempt}/{max_attempts})...")
            time.sleep(interval)
    return False


def load_sample_data():
    """Load sample data from sample-data.json."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    sample_path = os.path.join(script_dir, "sample-data.json")
    try:
        with open(sample_path, encoding="utf-8") as f:
            data = json.load(f)
        if not isinstance(data, list):
            data = [data]
        return data
    except FileNotFoundError:
        print(f"ERROR: sample-data.json not found at {sample_path}")
        return None
    except json.JSONDecodeError as e:
        print(f"ERROR: Invalid JSON in sample-data.json: {e}")
        return None


def send_test_data():
    """Load sample data and send each item to rootsmith upstream."""
    samples = load_sample_data()
    if not samples:
        return False

    # Use current timestamps so records fall within epoch's time range
    # (batch queries by time_start <= record.timestamp <= time_end)
    base_ts = int(time.time())
    print(f"Sending {len(samples)} records from sample-data.json to rootsmith upstream...")
    success_count = 0
    for i, payload in enumerate(samples):
        # Override timestamp so it falls within the next epoch window
        payload = dict(payload)
        payload["timestamp"] = base_ts - (len(samples) - i)
        try:
            req = Request(
                ROOTSMITH_UPSTREAM_URL,
                data=json.dumps(payload).encode(),
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            with urlopen(req, timeout=10) as resp:
                if 200 <= resp.status < 300:
                    success_count += 1
                else:
                    print(f"  Record {i + 1}: unexpected status {resp.status}")
        except HTTPError as e:
            print(f"  Record {i + 1}: failed {e.code} {e.reason}")
            return False
        except URLError as e:
            print(f"  Record {i + 1}: connection failed {e.reason}")
            return False

    print(f"Data sent successfully ({success_count}/{len(samples)} records)")
    return True


def main():
    print(f"Rootsmith upstream: {ROOTSMITH_UPSTREAM_URL}")
    print(f"Webhook server: http://0.0.0.0:{CLIENT_WEBHOOK_PORT}/webhook")
    print(f"Rootsmith will callback to: http://host.docker.internal:{CLIENT_WEBHOOK_PORT}/webhook")
    print("=" * 50)

    # Start webhook server in background
    server = HTTPServer(("0.0.0.0", CLIENT_WEBHOOK_PORT), WebhookHandler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    print(f"Webhook server listening on port {CLIENT_WEBHOOK_PORT}")

    # Wait for rootsmith
    if not wait_for_rootsmith():
        print("ERROR: Rootsmith not ready after max attempts")
        sys.exit(1)

    # Wait for epoch boundary (rootsmith commits every 10s; avoid sending right after commit)
    print("Waiting 5s for epoch alignment...")
    time.sleep(5)

    # Send test data
    if not send_test_data():
        sys.exit(1)

    print("Waiting for webhook callback (timeout 45s)...")

    # Wait for webhook (epoch=10s + mock delay ~1s; 45s covers 2 epoch cycles)
    if not webhook_received.wait(timeout=45):
        print("ERROR: Timeout - did not receive webhook from rootsmith")
        print("  Check: rootsmith logs for 'Failed to notify client' (host.docker.internal?)")
        sys.exit(1)

    print("Integration test passed!")
    sys.exit(0)


if __name__ == "__main__":
    main()
