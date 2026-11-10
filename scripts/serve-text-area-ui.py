"""Loopback-only browser fixture bridge to the real Rust area preparer."""
import argparse
import json
import subprocess
from http.server import BaseHTTPRequestHandler, HTTPServer

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--probe', required=True, help='Built text-area-ui-probe executable')
parser.add_argument('--port', type=int, default=1423)
args = parser.parse_args()


class Handler(BaseHTTPRequestHandler):
    def reply(self, value, status=200):
        body = json.dumps(value).encode('utf-8')
        self.send_response(status)
        self.send_header('Access-Control-Allow-Origin', 'http://127.0.0.1:1422')
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def run_probe(self, data=None):
        command = [args.probe] + (['--info'] if data is None else [])
        result = subprocess.run(command, input=data, capture_output=True, timeout=30,
                                creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        result.check_returncode()
        return json.loads(result.stdout)

    def do_GET(self):
        if self.path != '/info':
            return self.reply({'error': 'Unknown fixture route'}, 404)
        self.reply(self.run_probe())

    def do_POST(self):
        if self.path != '/prepare' or self.headers.get('Origin') != 'http://127.0.0.1:1422':
            return self.reply({'error': 'Unknown fixture request'}, 403)
        length = int(self.headers.get('Content-Length', '0'))
        if not 0 < length <= 65536:
            return self.reply({'error': 'Fixture request exceeds 64 KiB'}, 413)
        try:
            self.reply(self.run_probe(self.rfile.read(length)))
        except (ValueError, subprocess.SubprocessError) as error:
            self.reply({'error': str(error)}, 500)


print(f'Native area fixture on http://127.0.0.1:{args.port}', flush=True)
HTTPServer(('127.0.0.1', args.port), Handler).serve_forever()
