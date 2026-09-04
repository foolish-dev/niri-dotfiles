# niri-dotfiles

Personal niri/Wayland desktop config plus `dotctl`, the Rust binary in `src/`
that installs and deploys it. Single user, Arch and CachyOS.

## The hazard that is not obvious from the code

`~/.config/{niri,nvim,noctalia,kitty,…}` are **symlinks into this repo**, so the
tracked files *are* the running config. Three consequences:

- Editing a tracked config changes the live desktop immediately. There is no
  deploy step between you and a broken session.
- Apps write back through the symlink. `.config/noctalia/settings.json` is
  rewritten by the noctalia shell on every settings change — it goes dirty on
  its own, sometimes reindenting the whole file. Commit that churn as its own
  `chore(noctalia):` commit; do not normalise it to `.editorconfig`, because
  the next save undoes it and a conflicted tree hard-fails both commands that
  reach one: `dotctl all`, whose `git pull --ff-only --autostash` refuses a
  conflicted re-apply, and `dotctl deploy`, which refuses to symlink conflict
  markers into the live config. (The subcommands are `install`, `deploy` and
  `all` — there is no `dotctl update`.)


── Patch 1b — new bullet, "The hazard that is not obvious from the code",
   inserted between the noctalia bullet and the `deploy()` bullet (line 19) ──
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
- A hardware-specific package installed unconditionally. The XDNA NPU pack
  (`xrt`, `xrt-plugin-amdxdna`, `fastflowlm`, and the `limits.d` memlock
  drop-in) is gated on an XDNA function actually being on the PCI bus, and
  every step warns rather than `?`: ungated and fatal, a resolution failure on
  hardware the box does not have aborted `install()`, and under `dotctl all` a
  failed `install()` means `deploy()` never runs — the user gets no configs at
  all. Gate the next one the same way. (`iio-niri`, `wvkbd` and
  `rog-control-center` are still unconditional `?`s. That is the trap, not the
  precedent.)

## Tests

`cargo test` covers the Rust half. The shell half is four hermetic suites under
`tests/` — `hexstrike-assert-loopback.sh`, `mkproj.sh`, `config-syntax.sh`,
`dotfiles-check.sh` — and CI runs all four, plus `shellcheck` over
`.local/bin/*`, `tests/*.sh` and `.config/tmux/scripts/*.sh`. Run them before
you commit: `config-syntax.sh` is the only thing that parses the tracked
configs, and what it catches is a session that will not come up, not a red
build. Anything new in `.local/bin` has to be shellcheck-clean.

## Commits


── Patch 1d — new section, immediately before "## Commits" ──

Conventional commits, imperative subject. The body explains *why*, not what the
diff already shows; state what was observed and what was verified. Unrelated
changes go in separate commits.
