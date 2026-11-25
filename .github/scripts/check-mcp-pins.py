"""Fail when an MCP server the agent configs pin is not npm's `latest`, or is deprecated.

Dependabot cannot see these pins: they sit inside TOML and JSONC argument
lists, not in a manifest. And comparing version numbers would get the
reference servers backwards -- they moved from date versions (2026.8.31) to
semver (2.x), so a stale pin sorts *above* the fix. Upstream's own advice is
to install the `latest` dist-tag, so that is what each pin is held to.

Usage: python3 .github/scripts/check-mcp-pins.py   (from the repo root)
"""
import json
import os
import re
import sys
import urllib.parse
import urllib.request

CONFIGS = [".config/teleia/config.toml", ".config/opencode/opencode.json"]

# A quoted "name@version" or "@scope/name@version", wherever it sits. The
# version must start with a digit, so an address like "you@example.com" is
# not mistaken for one.
PIN = re.compile(r'"((?:@[a-z0-9._-]+/)?[a-z0-9._-]+)@([0-9][0-9A-Za-z.+-]*)"')


def npm_latest(pkg):
    url = "https://registry.npmjs.org/" + urllib.parse.quote(pkg, safe="@") + "/latest"
    with urllib.request.urlopen(url, timeout=30) as resp:
        return json.load(resp)


def main():
    pins = {}
    for path in CONFIGS:
        with open(path, encoding="utf-8") as fh:
            for pkg, ver in PIN.findall(fh.read()):
                pins.setdefault((pkg, ver), []).append(path)
    if not pins:
        print("no pins found in " + ", ".join(CONFIGS) + " -- has the format changed?")
        return 1

    rows, failed = [], 0
    for (pkg, ver), paths in sorted(pins.items()):
        try:
            meta = npm_latest(pkg)
        except Exception as exc:  # report every package, then fail once
            rows.append((pkg, ver, "?", f"lookup failed: {exc}", paths))
            failed += 1
            continue
        latest = meta.get("version", "?")
        problems = []
        if latest != ver:
            problems.append(f"npm's latest is {latest}")
        if meta.get("deprecated"):
            problems.append("deprecated: " + meta["deprecated"])
        failed += bool(problems)
        rows.append((pkg, ver, latest, "; ".join(problems) or "ok", paths))

    for pkg, ver, latest, verdict, paths in rows:
        mark = "ok  " if verdict == "ok" else "FAIL"
        print(f"{mark}  {pkg}@{ver}  (latest {latest})  {verdict if verdict != 'ok' else ''}")
        if verdict != "ok":
            print("      pinned in: " + ", ".join(paths))
    print(f"\n{len(rows)} pin(s) checked, {failed} need attention")

    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as fh:
            fh.write("| Package | Pinned | npm latest | Verdict |\n|---|---|---|---|\n")
            for pkg, ver, latest, verdict, _ in rows:
                fh.write(f"| `{pkg}` | `{ver}` | `{latest}` | {verdict} |\n")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
