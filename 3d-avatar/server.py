#!/usr/bin/env python3
"""Ava 3D avatar — local dev server.

Serves the avatar page AND proxies /api/* to LM Studio (localhost:1234)
so the browser can talk to the LLM without CORS issues.

Usage:
    python3 server.py [--port 8000]
"""
import argparse
import http.server
import json
import os
import socketserver
import sys
import urllib.request
import urllib.error

LM_STUDIO = "http://localhost:1234"
PORT = 8000
STATIC_DIR = os.path.dirname(os.path.abspath(__file__))


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=STATIC_DIR, **kwargs)

    # ---- proxy: /api/* -> LM Studio ----
    def _proxy(self):
        target = LM_STUDIO + self.path[len("/api"):]
        body = None
        headers = {}
        if self.headers.get("Content-Type"):
            headers["Content-Type"] = self.headers["Content-Type"]
        if self.command in ("POST", "PUT", "PATCH"):
            length = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(length) if length else b"{}"
        req = urllib.request.Request(target, data=body, method=self.command, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                data = resp.read()
                self.send_response(resp.status)
                self.send_header("Content-Type", resp.headers.get("Content-Type", "application/json"))
                self.send_header("Access-Control-Allow-Origin", "*")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
        except urllib.error.HTTPError as e:
            data = e.read()
            self.send_response(e.code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except Exception as e:  # LM Studio not running
            payload = json.dumps({"error": f"LM Studio unreachable: {e}"}).encode()
            self.send_response(502)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    def do_GET(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        return super().do_GET()

    def do_POST(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        self.send_response(404); self.end_headers()

    def end_headers(self):
        # never cache the demo page so fixes always reach the browser
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        super().end_headers()

    def log_message(self, fmt, *args):  # quieter logs
        sys.stderr.write("%s %s\n" % (self.address_string(), fmt % args))


def main():
    global PORT
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", type=int, default=PORT)
    args = ap.parse_args()
    PORT = args.port
    with socketserver.ThreadingTCPServer(("127.0.0.1", PORT), Handler) as httpd:
        print(f"Ava avatar running at http://localhost:{PORT}  (LLM proxy -> {LM_STUDIO})")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nbye!")


if __name__ == "__main__":
    main()
