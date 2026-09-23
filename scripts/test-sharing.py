#!/usr/bin/env python3
"""Check the live sharing lifecycle with synthetic content; never print credentials."""
import json
import secrets
import subprocess
import urllib.error
import urllib.request

BASE = "https://redrule.vercel.app"
def sharing_key():
    for service in ("Redrule", "Minutes"):
        found = subprocess.run(
            ["security", "find-generic-password", "-s", service, "-a", "sharing", "-w"],
            capture_output=True, text=True,
        )
        if found.returncode == 0 and found.stdout.strip():
            return found.stdout.strip()
    raise SystemExit("No sharing key in the Keychain under Redrule or the previous service.")

key = sharing_key()
share_id = secrets.token_hex(32)
api_path = "/api/share?id=" + share_id
page_path = "/s/" + share_id


def request(path, method="GET", body=None, authenticate=False):
    headers = {"Content-Type": "application/json"}
    if authenticate:
        headers["Authorization"] = "Bearer " + key
    req = urllib.request.Request(
        BASE + path, data=None if body is None else json.dumps(body).encode(),
        headers=headers, method=method,
    )
    try:
        with urllib.request.urlopen(req, timeout=45) as response:
            return response.status, response.read().decode(), response.headers
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode(), error.headers


payload = {"note": {
    "title": "Redrule synthetic sharing check", "tldr": "Synthetic content only.",
    "sections": [], "decisions": [], "actionItems": [],
}}
try:
    status, _, _ = request(api_path, "PUT", payload)
    assert status == 401, f"Unauthenticated write: expected 401, got {status}"
    status, _, _ = request(api_path, "PUT", payload, True)
    assert status == 200, f"Publish: expected 200, got {status}"
    status, html, headers = request(page_path)
    assert status == 200 and payload["note"]["title"] in html, f"Read failed ({status})"
    assert "no-store" in headers.get("Cache-Control", ""), "Missing no-store policy"
    payload["transcript"] = [{"speaker": "Synthetic Speaker", "time": "00:01", "text": "Synthetic passage."}]
    status, _, _ = request(api_path, "PUT", payload, True)
    assert status == 200, f"Update failed ({status})"
    status, html, _ = request(page_path)
    assert status == 200 and "Synthetic passage." in html, "Transcript update missing"
    del payload["transcript"]
    status, _, _ = request(api_path, "PUT", payload, True)
    assert status == 200, f"Transcript removal failed ({status})"
    status, html, _ = request(page_path)
    assert status == 200 and "Synthetic passage." not in html, "Removed transcript still visible"
finally:
    status, _, _ = request(api_path, "DELETE", authenticate=True)
    assert status == 204, f"Synthetic share cleanup failed ({status}); share ID: {share_id}"
status, _, _ = request(page_path)
assert status == 404, f"Revocation: expected 404, got {status}"
print("PASS: unauthorized writes blocked; publish, public read, update, transcript removal, and revocation verified. Synthetic share deleted.")
