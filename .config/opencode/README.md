# OpenCode Configuration

OpenCode ([opencode.ai](https://opencode.ai)) agent + provider + MCP setup for this dotfiles environment. Authoritative config lives in [`opencode.json`](./opencode.json).

## Layout

```text
.config/opencode/
├── opencode.json         # providers, MCP servers, agents
├── agent/                # agent markdown (prompt + frontmatter)
│   ├── docs.md           # mirrored from heimdall_opencode
│   └── git-committer.md  # mirrored from heimdall_opencode
├── agents.json           # legacy agent metadata (kept for reference)
├── heimdall_opencode/    # git submodule -- SuperClaude-Org/heimdall_opencode @ dev
└── package.json          # auto-generated when opencode installs @opencode-ai/plugin
```

`heimdall_opencode/` is a pinned, shallow submodule. It's the upstream source for the agents in `agent/` and the inspiration for the `weather` MCP demo in `opencode.json`. Refresh it with:

```bash
git submodule update --remote --merge .config/opencode/heimdall_opencode
```

## Providers

Both providers are local and must be running before use.

| Key | Endpoint | Use |
|---|---|---|
| `ollama` | `http://127.0.0.1:11434/v1` | Default (`jaahas/crow:9b` tool-capable) |
| `lmstudio` | `http://127.0.0.1:1234/v1` | Crow 9B Heretic variants + Qwen3 14B (see `opencode.json` for capabilities) |

Commented-out stubs for `anthropic` and `google` exist in `opencode.json` for when API keys are available.

## MCP Servers

| Server | Type | Notes |
|---|---|---|
| `hexstrike-ai` | local stdio | Bridges to the Flask backend on `127.0.0.1:8888` (managed by `hexstrike-server.service`) |
| `context7` | remote (streamable HTTP) | Library docs & code samples — `https://mcp.context7.com/mcp` (bearer auth via `CONTEXT7_API_KEY`, anonymous also works) |
| `filesystem` | local stdio | `@modelcontextprotocol/server-filesystem`, rooted at `$HOME` |
| `github` | local stdio | `@modelcontextprotocol/server-github` (needs `GITHUB_PERSONAL_ACCESS_TOKEN`) |
| `fetch` | local stdio | `@tokenizin/mcp-npx-fetch` — generic HTTP fetcher |
| `playwright` | local stdio | `@playwright/mcp@latest` — headless browser automation |
| `sequential-thinking` | local stdio | `@modelcontextprotocol/server-sequential-thinking` — multi-step reasoning |
| `memory` | local stdio | `@modelcontextprotocol/server-memory` — session memory store |
| `git` | local stdio | `mcp-server-git` from `~/.local/bin` |
| `weather` | local stdio | `@h1deya/mcp-server-weather` via `opencode x` (demo, from heimdall_opencode) |

## Agents

| Key | Mode | Tools | Purpose |
|---|---|---|---|
| `hexstrike-analyst-context7` | primary | hexstrike-ai, context7 | Authorized pentest / CTF recon with HexStrike + docs lookup |
| `superclaude-architect-context7` | primary | context7 | Codebase architect with Context7 library research |
| `build` | primary | hexstrike-ai, context7 | Build systems, CI/CD, infrastructure code |
| `analyze` | primary | hexstrike-ai, context7 | Security-focused code review |
| `docs` | subagent | — | Documentation writer (from heimdall_opencode) |
| `git-committer` | subagent | — | Conventional commits + push helper (from heimdall_opencode) |

Invoke from chat with `@<key>` (e.g. `@build`, `@git-committer`). Don't prefix the key with `@` in `opencode.json`.

## HexStrike Quick Reference

Once the `hexstrike-server.service` is running, the `hexstrike-analyst-context7` agent can drive 150+ security tools. A few common commands the agent has at its disposal:

| Tool | Example |
|---|---|
| `rustscan` / `nmap` / `masscan` | `rustscan -r 192.168.1.0/24 --top-ports 1000` |
| `nuclei` / `nikto` / `zaproxy` | `nuclei -u https://target -severity high,medium` |
| `sqlmap` | `sqlmap -u "http://target/page?id=1"` |
| `ffuf` / `gobuster` / `dirsearch` | `ffuf -w wordlist.txt -u https://target/FUZZ` |
| `wafw00f` | `wafw00f target.com` |

**Authorization reminder:** only scan systems you own or have written permission to test.

## Troubleshooting

```bash
# HexStrike backend
systemctl --user status hexstrike-server
systemctl --user restart hexstrike-server
curl -s http://127.0.0.1:8888/health | jq '.'

# LM Studio
lms-status                        # curl /v1/models
lms-server                        # start headless server on :1234

# Submodule empty after a plain `git clone`
git submodule update --init --recursive --depth 1
```

For the full dotfiles setup guide see the [repo README](../../README.md).
