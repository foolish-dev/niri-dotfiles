"""tests/hexstrike-host-guard.py -- run the shipped Host guard against Flask.

assets/hexstrike-host-guard.py is what `dotctl install` pastes into HexStrike's
server, right after `app = Flask(__name__)`, so that a web page which DNS-rebinds
its own hostname to 127.0.0.1 cannot drive /api/command: the page cannot change
the Host header, and the guard answers only loopback names. The Rust tests pin
where the guard goes; this pins what it does, on a real Flask.

No socket is opened and HexStrike is not imported: a throwaway app gets the
guard and a stand-in /api/command, and Flask's test client drives it.

Usage: python3 tests/hexstrike-host-guard.py
       REQUIRE_FLASK=1 makes a missing Flask a failure (CI installs the version
       HexStrike's venv runs); otherwise it is reported as SKIP.
"""
import os
import sys
from pathlib import Path

GUARD = Path(__file__).resolve().parent.parent / "assets" / "hexstrike-host-guard.py"

try:
    import flask
    from importlib.metadata import version
except ImportError:
    if os.environ.get("REQUIRE_FLASK"):
        print("FAIL  flask is not installed, and REQUIRE_FLASK is set")
        sys.exit(1)
    print("SKIP  flask is not installed (pip install flask==3.0.3 to run this)")
    sys.exit(0)

app = flask.Flask("hexstrike-guard-test")
ran = []


@app.route("/api/command", methods=["POST"])
def command():
    ran.append(flask.request.json)
    return flask.jsonify({"ran": True})


@app.route("/health")
def health():
    return flask.jsonify({"ok": True})


exec(compile(GUARD.read_text(encoding="utf-8"), str(GUARD), "exec"), {"app": app})
client = app.test_client()

cases = failed = 0


def case(name, ok, detail=""):
    global cases, failed
    cases += 1
    if ok:
        print(f"PASS  {name}")
    else:
        failed += 1
        print(f"FAIL  {name}\n        {detail}")


print(f"flask {version('flask')}, werkzeug {version('werkzeug')}")
for host in ("127.0.0.1:8888", "127.0.0.1", "localhost:8888", "[::1]:8888"):
    got = client.get("/health", headers={"Host": host}).status_code
    case(f"loopback_name_{host}_is_answered", got == 200, f"got {got}")

for host in ("evil.example:8888", "127.0.0.1.evil.example:8888",
             "localhost.evil.example", "0.0.0.0:8888", "[::1"):
    got = client.get("/health", headers={"Host": host}).status_code
    # Werkzeug may reject a malformed Host itself (400); either way, not 200.
    case(f"foreign_name_{host}_is_refused", got in (400, 403), f"got {got}")

r = client.post("/api/command", json={"command": "id"}, headers={"Host": "rebind.example:8888"})
case("a_rebound_post_never_reaches_api_command", r.status_code == 403 and not ran,
     f"status {r.status_code}, view ran: {bool(ran)}")

r = client.post("/api/command", json={"command": "id"}, headers={"Host": "127.0.0.1:8888"})
case("a_loopback_post_still_reaches_api_command", r.status_code == 200 and ran,
     f"status {r.status_code}, view ran: {bool(ran)}")

print(f"\n{cases} case(s), {failed} failed")
sys.exit(1 if failed else 0)
