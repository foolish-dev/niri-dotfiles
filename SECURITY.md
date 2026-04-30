# Security policy

This repository installs offensive security tooling (BlackArch + 150+ tools
via HexStrike AI MCP). Please read this before running anything on a box
you care about.

## What this repo does to your system

`install.sh` (via `bootstrap.sh`):

- Adds **two third-party pacman repos** to `/etc/pacman.conf`:
  - [BlackArch](https://blackarch.org) — imports their strap via
    `curl -sL https://blackarch.org/strap.sh | sudo ...`
  - [Chaotic AUR](https://aur.chaotic.cx) — imports their keyring + mirrorlist
- Installs **~370 packages** (many more transitively via BlackArch metapackages), many privileged (Metasploit, msfvenom, aircrack,
  tcpdump, ettercap, mimikatz, etc.)
- Adds your user to the `docker` and `wireshark` groups (effectively root for
  container escape / raw packet capture)
- Enables **systemd units** at boot: NetworkManager, iwd, bluetooth, docker,
  sddm
- Clones and runs [HexStrike AI](https://github.com/0x4m4/hexstrike-ai)
  locally, enabling a user service on `127.0.0.1:8888`

`deploy.sh`:

- Symlinks configs into `~/.config/`, shell config into `~/.zshrc`,
  scripts into `~/.local/bin/`
- Copies system-wide SDDM config into `/etc/sddm.conf.d/` (requires sudo)
- With `DEPLOY_LOADER=1`, copies `systemd-boot` loader entries into
  `/boot/loader/` — the tracked entries reference a specific PARTUUID + kernel
  cmdline. Applying those on a different install leaves the box unbootable.

## Reporting a vulnerability

Open a private advisory on GitHub:
<https://github.com/foolish-dev/niri-dotfiles/security/advisories/new>

Or email <cardoffools@gmail.com>. Please include:

- Affected file(s) / function(s)
- Reproduction steps
- Impact assessment (what access is gained or what's bypassed)

You can expect an acknowledgement within a week. Because this is a personal
dotfiles repo and not production infrastructure, there is no formal SLA —
but security issues are taken seriously and will be patched quickly.

## Safe-use recommendations

- **Read before piping:** the `curl | bash` one-liner in the README is a
  convenience. For first-time installs, clone the repo and inspect both
  `install.sh` and `deploy.sh` before running.
- **Run in a VM first:** this configuration is designed for a dedicated
  offensive-security workstation. Do not deploy on your daily-driver box or
  work laptop unless you understand the surface area.
- **Verify third-party keys:** `install.sh` imports the BlackArch strap and
  Chaotic AUR keyring unconditionally. If either project is compromised,
  every downstream user (including this dotfiles installer) inherits the
  compromise. Consider pinning keys after the first successful install.
- **Review `hexstrike-ai` before enabling:** the MCP server runs with your
  user's privileges and can invoke most installed security tools. It's
  loopback-only by default (`127.0.0.1:8888`) — do not expose it to a
  network you don't control.

## Known trade-offs

- No signature verification on the `bootstrap.sh` one-liner (pending release
  tagging).
- No SBOM / dependency tracking for the ~370 explicit packages (plus their
  BlackArch metapackage dependency closure) installed — you inherit whatever
  is in BlackArch and Chaotic AUR at install time.
- `.gitconfig` and SSH keys are the user's responsibility. `deploy.sh`
  generates `~/.gitconfig.local` but does not touch `~/.ssh/`.
