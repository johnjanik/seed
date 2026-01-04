#!/usr/bin/env python3
"""Simple HTTP server for the Seed WASM demo.

Usage: python serve.py [port]

Opens the demo in your default browser.
"""

import http.server
import socketserver
import webbrowser
import os
import sys

def find_available_port(start_port=8080, max_attempts=20):
    """Find an available port starting from start_port."""
    import socket
    for port in range(start_port, start_port + max_attempts):
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.bind(('', port))
                return port
        except OSError:
            continue
    raise RuntimeError(f"No available port found in range {start_port}-{start_port + max_attempts}")

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else find_available_port()

# Change to the seed-wasm directory (parent of demo)
os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

class CORSRequestHandler(http.server.SimpleHTTPRequestHandler):
    """HTTP handler with CORS and proper MIME types for WASM."""

    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        '.wasm': 'application/wasm',
        '.js': 'application/javascript',
        '.mjs': 'application/javascript',
    }

    def end_headers(self):
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        super().end_headers()

    def log_message(self, format, *args):
        # Quieter logging
        if '200' not in str(args):
            print(f"  {args[0]}")

print(f"\n  Seed Engine Demo")
print(f"  ================")
print(f"  Server: http://localhost:{PORT}")
print(f"  Demo:   http://localhost:{PORT}/demo/")
print(f"\n  Press Ctrl+C to stop\n")

# Open browser
webbrowser.open(f'http://localhost:{PORT}/demo/')

socketserver.TCPServer.allow_reuse_address = True
with socketserver.TCPServer(("", PORT), CORSRequestHandler) as httpd:
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n  Server stopped.")
