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
opt.clipboard   = "unnamedplus"

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
