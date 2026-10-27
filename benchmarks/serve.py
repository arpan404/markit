from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
import sys
class Handler(SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Cross-Origin-Opener-Policy','same-origin')
        self.send_header('Cross-Origin-Embedder-Policy','require-corp')
        self.send_header('Cache-Control','no-store')
        super().end_headers()
    def log_message(self, *args):
        pass
ThreadingHTTPServer(('127.0.0.1',int(sys.argv[1])), Handler).serve_forever()
