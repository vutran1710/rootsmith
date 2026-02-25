#!/usr/bin/env python3
"""
Mock external service that:
1. Receives protobuf commit requests from rootsmith
2. Sends webhook callback with commitment result
"""

import json
import os
import time
import threading
from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.request

ROOTSMITH_WEBHOOK_URL = os.environ.get(
    "ROOTSMITH_WEBHOOK_URL",
    "http://localhost:9000/webhook/commitment"
)


class MockExternalHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length)

        print(f"\n{'='*50}")
        print(f"Received request: {self.path}")
        print(f"Content-Type: {self.headers.get('Content-Type')}")
        print(f"Body size: {len(body)} bytes")

        # Extract webhook_url from multipart if present (simplified parsing)
        webhook_url = ROOTSMITH_WEBHOOK_URL
        if b"webhook_url" in body:
            print("Found webhook_url in request")

        # Generate job_id
        job_id = f"job-{int(time.time())}"
        print(f"Generated job_id: {job_id}")

        # Respond with job_id
        response = json.dumps({"job_id": job_id}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", len(response))
        self.end_headers()
        self.wfile.write(response)

        # Send webhook callback after a short delay (simulates async processing)
        threading.Thread(
            target=send_webhook_callback,
            args=(webhook_url, job_id),
            daemon=True
        ).start()

    def log_message(self, format, *args):
        print(f"[HTTP] {args[0]}")


def send_webhook_callback(webhook_url: str, job_id: str):
    """Send commitment result back to rootsmith via webhook."""
    time.sleep(1)  # Simulate processing time

    # Commitment.root expects Vec<u8> = JSON array of bytes, not hex string
    root_hex = "deadbeefcafe1234567890abcdef"
    root_bytes = list(bytes.fromhex(root_hex.ljust(64, "0")[:64]))  # 32 bytes

    timestamp = int(time.time())
    payload = {
        "job_id": job_id,
        "status": "success",
        "commitment": {
            "root": root_bytes,
            "namespaces": [],
            "committed_at": timestamp
        },
        "item_count": 1,
        "timestamp": timestamp,
        "proofs": {},
        "meta": {"provider": "mock-external-service"}
    }

    print(f"\n{'='*50}")
    print(f"Sending webhook to: {webhook_url}")
    print(f"Payload: {json.dumps(payload, indent=2)}")

    try:
        req = urllib.request.Request(
            webhook_url,
            data=json.dumps(payload).encode(),
            headers={"Content-Type": "application/json"},
            method="POST"
        )
        with urllib.request.urlopen(req, timeout=10) as resp:
            response_body = resp.read().decode()
            print(f"Webhook response: {resp.status} - {response_body}")
    except Exception as e:
        print(f"Webhook failed: {e}")


def main():
    port = 3000
    server = HTTPServer(("0.0.0.0", port), MockExternalHandler)
    print(f"Mock external service listening on http://localhost:{port}")
    print(f"Will send webhooks to: {ROOTSMITH_WEBHOOK_URL}")
    print("=" * 50)
    server.serve_forever()


if __name__ == "__main__":
    main()
