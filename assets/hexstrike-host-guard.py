# dotctl: loopback-Host guard. `dotctl install` inserts this right after
# `app = Flask(__name__)` in hexstrike_server.py and re-applies it on every
# run, as it does the bind rewrite.
#
# The loopback bind keeps other machines off the API; it does nothing about the
# browser on this one. A page that DNS-rebinds its own hostname to 127.0.0.1
# reaches the port same-origin, so no CORS preflight stands in its way, and
# `/api/command` runs whatever it is sent. What the page cannot change is the
# Host header, which still carries its own name. So answer only requests
# addressed to a loopback name.
from flask import jsonify as _dotctl_jsonify, request as _dotctl_request


@app.before_request
def _dotctl_loopback_host_only():
    host = _dotctl_request.host
    if host.startswith("["):
        host = host[: host.find("]") + 1]
    else:
        host = host.split(":", 1)[0]
    if host not in ("127.0.0.1", "localhost", "[::1]"):
        return _dotctl_jsonify({"error": "dotctl: Host must be a loopback name"}), 403
    return None
