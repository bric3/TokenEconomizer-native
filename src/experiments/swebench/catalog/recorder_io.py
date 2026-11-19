"""HTTP/TLS and process I/O for the native catalogue recorder."""
import json
import os
import subprocess
import sys
import threading
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

port, out, upstream = sys.argv[1:]
engine = subprocess.Popen([os.environ.get("TE_CATALOG_HELPER", "/tmp/te-net-guard"),
    "--catalog-recorder", out, upstream], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
    text=True, encoding="utf-8")
lock = threading.Lock()

def policy(request):
    with lock:
        engine.stdin.write(json.dumps(request) + "\n")
        engine.stdin.flush()
        return json.loads(engine.stdout.readline())

class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def log_message(self, *args): pass
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("content-length") or 0))
        try: document = json.dumps(json.loads(body))
        except Exception: document = None
        self.forward(body, document)
    def do_GET(self): self.forward(None, None)
    def forward(self, body, document):
        prepared = policy(dict(op="request", method=self.command, path=self.path,
            headers=list(self.headers.items()), document=document))
        request = urllib.request.Request(prepared["url"], data=body, method=self.command)
        for name, value in prepared["headers"]: request.add_header(name, value)
        try:
            with urllib.request.urlopen(request, timeout=300) as response:
                self.send_response(response.status)
                for name, value in policy(dict(op="response", headers=list(response.headers.items()))):
                    self.send_header(name, value)
                self.end_headers()
                while True:
                    chunk = response.read(4096)
                    if not chunk: break
                    self.wfile.write(b"%X\r\n%s\r\n" % (len(chunk), chunk))
                    self.wfile.flush()
                self.wfile.write(b"0\r\n\r\n")
        except urllib.error.HTTPError as error:
            self.reply(policy(dict(op="error", status=error.code, body=list(error.read()))))
        except Exception as error:
            self.reply(policy(dict(op="error", message=str(error))))
    def reply(self, response):
        self.send_response(response["status"])
        for name, value in response["headers"]: self.send_header(name, value)
        self.end_headers()
        self.wfile.write(bytes(response["body"]))

try: ThreadingHTTPServer(("127.0.0.1", int(port)), Handler).serve_forever()
finally:
    engine.stdin.close()
    engine.wait()
