#!/usr/bin/env python3
"""Servidor HTTP local mínimo para pruebas de perfkit.

Responde 200 con un cuerpo JSON a cualquier método/ruta, con campos
extraíbles (token, id, slideshow.title) para ejercitar assertions/extractores.

Uso: python3 tools/test-server.py [puerto]   (por defecto 8787)
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def _respond(self):
        body = json.dumps(
            {
                "ok": True,
                "id": 123,
                "token": "abc123",
                "slideshow": {"title": "Sample"},
                "path": self.path,
                "method": self.command,
            }
        ).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        try:
            self.wfile.write(body)
        except BrokenPipeError:
            pass

    def _drain(self):
        length = int(self.headers.get("Content-Length", 0) or 0)
        if length:
            self.rfile.read(length)

    def do_GET(self):
        self._respond()

    def do_HEAD(self):
        self._respond()

    def do_POST(self):
        self._drain()
        self._respond()

    do_PUT = do_POST
    do_PATCH = do_POST
    do_DELETE = do_GET

    def log_message(self, *args):
        pass  # silencio


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8787
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
