# niri-dotfiles

Personal niri/Wayland desktop config plus `dotctl`, the Rust binary in `src/`
that installs and deploys it. Single user, Arch and CachyOS.

## Coding guidelines

[Karpathy.md](Karpathy.md) is the standing brief for Rust work here — read it
before changing `src/main.rs`.

## The hazard that is not obvious from the code

`~/.config/{niri,nvim,noctalia,kitty,…}` are **symlinks into this repo**, so the
tracked files *are* the running config. Three consequences:

- Editing a tracked config changes the live desktop immediately. There is no
  deploy step between you and a broken session.
- Apps write back through the symlink. `.config/noctalia/settings.json` is
  rewritten by the noctalia shell on every settings change — it goes dirty on
  its own, sometimes reindenting the whole file. Commit that churn as its own
  `chore(noctalia):` commit; do not normalise it to `.editorconfig`, because
  the next save undoes it and a permanently conflicted tree hard-fails
  `dotctl update`.
- **Never call `deploy()` from a test.** It reads `$HOME` and symlinks into it.
  Test `link_dotfiles(repo, home)` against a temp dir instead — that is why the
  filesystem half is a separate function. `deploy()` also runs
  `systemctl --user`, which reaches the real session whatever `home` says.

## Fresh-install invariants

A fresh clone must produce a bootable desktop. Things that broke this before:

- A mandatory `include` of a generated, gitignored file. `config.kdl` uses
  `optional=true` for `grogu.kdl`; `colorscheme.lua` falls back to
  `tokyonight-night`. Keep it that way.
- `hexstrike-server` is an **unauthenticated** API whose `/api/command` runs
  arbitrary shell. It must never bind off loopback. Two independent guards:
  `dotctl install` rewrites upstream's `app.run(host=…)`, and the unit's
  `ExecStartPost=` checks the socket itself and stops the service otherwise.
  Both are deliberately fail-closed — do not soften either into a warning.

## Commits

Conventional commits, imperative subject. The body explains *why*, not what the
diff already shows; state what was observed and what was verified. Unrelated
changes go in separate commits.
