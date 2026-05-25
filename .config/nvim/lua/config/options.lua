-- =============================================================================
-- Core Neovim Options
-- =============================================================================
local opt = vim.opt

-- ── UI ────────────────────────────────────────────────────────────────────
opt.number         = true
opt.relativenumber = true
opt.cursorline     = true
opt.signcolumn     = "yes"
opt.termguicolors  = true
opt.showmode       = false         -- shown by statusline
opt.pumheight      = 12
opt.scrolloff      = 8
opt.sidescrolloff  = 8
opt.splitbelow     = true
opt.splitright     = true
opt.laststatus     = 3             -- global statusline
opt.cmdheight      = 1
opt.winminwidth    = 5
opt.wrap           = false
opt.linebreak      = true
opt.fillchars      = { eob = " ", fold = " ", foldopen = "▾", foldclose = "▸" }

-- ── Editing ───────────────────────────────────────────────────────────────
opt.expandtab   = true
opt.shiftwidth  = 4
opt.tabstop     = 4
opt.softtabstop = 4
opt.smartindent = true
opt.shiftround  = true

-- ── Search ────────────────────────────────────────────────────────────────
opt.ignorecase  = true
opt.smartcase   = true
opt.hlsearch    = true
opt.incsearch   = true
opt.grepprg     = "rg --vimgrep --smart-case"
opt.grepformat  = "%f:%l:%c:%m"

-- ── Files ─────────────────────────────────────────────────────────────────
opt.undofile    = true
opt.undolevels  = 10000
opt.swapfile    = false
opt.backup      = false
opt.writebackup = false
opt.autoread    = true

-- ── Completion ────────────────────────────────────────────────────────────
opt.completeopt = { "menu", "menuone", "noselect" }
opt.wildmode    = "longest:full,full"

-- ── Timing ────────────────────────────────────────────────────────────────
opt.updatetime  = 200
opt.timeoutlen  = 400

-- ── Clipboard (system) ───────────────────────────────────────────────────
-- Pin the wl-clipboard provider explicitly when wl-copy is on $PATH.
-- Avoids nvim's auto-detect picking the wrong invocation under tmux and
-- the "clipboard: error invoking 'wl-copy'" failure when wl-paste tries
-- to decode non-text MIME types. cache_enabled stops wl-paste being
-- respawned on every paste call.
if vim.fn.executable("wl-copy") == 1 then
  vim.g.clipboard = {
    name  = "wl-clipboard",
    copy  = {
      ["+"] = { "wl-copy", "--type", "text/plain" },
      ["*"] = { "wl-copy", "--primary", "--type", "text/plain" },
    },
    paste = {
      ["+"] = { "wl-paste", "--no-newline" },
      ["*"] = { "wl-paste", "--no-newline", "--primary" },
    },
    cache_enabled = 1,
  }
end
opt.clipboard = "unnamedplus"

-- ── Fold (treesitter-based) ──────────────────────────────────────────────
opt.foldmethod  = "expr"
opt.foldexpr    = "v:lua.vim.treesitter.foldexpr()"
opt.foldlevel   = 99
opt.foldlevelstart = 99

-- ── Misc ──────────────────────────────────────────────────────────────────
opt.mouse        = "a"
opt.confirm      = true
opt.conceallevel = 2
opt.formatoptions:remove("o")
vim.g.markdown_recommended_style = 0

-- ── Disable unused language providers ────────────────────────────────────
-- Install pynvim (python-pynvim) and flip loaded_python3_provider to 1 if
-- you need remote Python plugins.
vim.g.loaded_python3_provider = 0
vim.g.loaded_node_provider    = 0
vim.g.loaded_perl_provider    = 0
vim.g.loaded_ruby_provider    = 0
