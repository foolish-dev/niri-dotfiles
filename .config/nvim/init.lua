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
  checker = { enabled = true, notify = false },
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
