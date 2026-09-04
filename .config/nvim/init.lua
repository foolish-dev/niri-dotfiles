-- =============================================================================
-- Neovim Config -- Coding & Cybersecurity Workstation
-- ~/.config/nvim/init.lua
-- =============================================================================

-- ── Leader key (before lazy) ──────────────────────────────────────────────
vim.g.mapleader = " "
vim.g.maplocalleader = " "

-- ── Core options ──────────────────────────────────────────────────────────
require("config.options")
require("config.keymaps")
require("config.autocmds")

-- ── Bootstrap lazy.nvim ───────────────────────────────────────────────────
local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"
if not (vim.uv or vim.loop).fs_stat(lazypath) then
  vim.fn.system({
    "git",
    "clone",
    "--filter=blob:none",
    "https://github.com/folke/lazy.nvim.git",
    "--branch=stable",
    lazypath,
  })
end
vim.opt.rtp:prepend(lazypath)

-- ── Load plugins ──────────────────────────────────────────────────────────
require("lazy").setup("plugins", {
  defaults = { lazy = true },
  install = { colorscheme = { "tokyonight" } },
  -- Left off (upstream default): with it on, lazy registers a VeryLazy
  -- autocmd whose first check is scheduled at max(last_check + 3600 - now, 0),
  -- so the first nvim of any day git-fetches all 108 plugins at once -- the
  -- only thing here that touches the network on a plain start. Versions are
  -- pinned in the committed lazy-lock.json, so :Lazy check on demand is enough.
  checker = { enabled = false },
  change_detection = { notify = false },
  performance = {
    rtp = {
      disabled_plugins = {
        "gzip",
        "tarPlugin",
        "zipPlugin",
        "tohtml",
        "netrwPlugin",
        "tutor",
      },
    },
  },
})
