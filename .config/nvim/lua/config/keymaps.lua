-- =============================================================================
-- Keymaps
-- =============================================================================
local map = vim.keymap.set

-- ── Better movement ───────────────────────────────────────────────────────
map("n", "j", "v:count == 0 ? 'gj' : 'j'", { expr = true, silent = true })
map("n", "k", "v:count == 0 ? 'gk' : 'k'", { expr = true, silent = true })

-- ── Window navigation ─────────────────────────────────────────────────────
map("n", "<C-h>", "<C-w>h", { desc = "Go to left window" })
map("n", "<C-j>", "<C-w>j", { desc = "Go to lower window" })
map("n", "<C-k>", "<C-w>k", { desc = "Go to upper window" })
map("n", "<C-l>", "<C-w>l", { desc = "Go to right window" })

-- ── Resize windows ────────────────────────────────────────────────────────
map("n", "<C-Up>",    "<cmd>resize +2<cr>",          { desc = "Increase height" })
map("n", "<C-Down>",  "<cmd>resize -2<cr>",          { desc = "Decrease height" })
map("n", "<C-Left>",  "<cmd>vertical resize -2<cr>", { desc = "Decrease width" })
map("n", "<C-Right>", "<cmd>vertical resize +2<cr>", { desc = "Increase width" })

-- ── Buffer navigation ─────────────────────────────────────────────────────
map("n", "<S-h>", "<cmd>bprevious<cr>", { desc = "Prev buffer" })
map("n", "<S-l>", "<cmd>bnext<cr>",     { desc = "Next buffer" })
map("n", "<leader>bd", "<cmd>bdelete<cr>", { desc = "Delete buffer" })
map("n", "<leader>bD", "<cmd>%bdelete<cr>", { desc = "Delete all buffers" })

-- ── Move lines ────────────────────────────────────────────────────────────
map("n", "<A-j>", "<cmd>m .+1<cr>==",        { desc = "Move line down" })
map("n", "<A-k>", "<cmd>m .-2<cr>==",        { desc = "Move line up" })
map("v", "<A-j>", ":m '>+1<cr>gv=gv",        { desc = "Move selection down" })
map("v", "<A-k>", ":m '<-2<cr>gv=gv",        { desc = "Move selection up" })
map("i", "<A-j>", "<esc><cmd>m .+1<cr>==gi", { desc = "Move line down" })
map("i", "<A-k>", "<esc><cmd>m .-2<cr>==gi", { desc = "Move line up" })

-- ── Better indenting ─────────────────────────────────────────────────────
map("v", "<", "<gv")
map("v", ">", ">gv")

-- ── Clear search ──────────────────────────────────────────────────────────
map({ "i", "n" }, "<esc>", "<cmd>noh<cr><esc>", { desc = "Escape and clear hlsearch" })

-- ── Save / Quit ───────────────────────────────────────────────────────────
map("n", "<leader>w",  "<cmd>w<cr>",  { desc = "Save" })
map("n", "<leader>q",  "<cmd>q<cr>",  { desc = "Quit" })
map("n", "<leader>Q",  "<cmd>qa!<cr>", { desc = "Force quit all" })
map("n", "<C-s>",      "<cmd>w<cr>",  { desc = "Save" })

-- ── Diagnostic navigation ─────────────────────────────────────────────────
map("n", "[d", function() vim.diagnostic.jump({ count = -1, float = true }) end, { desc = "Prev diagnostic" })
map("n", "]d", function() vim.diagnostic.jump({ count =  1, float = true }) end, { desc = "Next diagnostic" })
map("n", "<leader>e", vim.diagnostic.open_float, { desc = "Line diagnostics" })

-- ── Terminal ──────────────────────────────────────────────────────────────
map("t", "<Esc><Esc>", "<C-\\><C-n>", { desc = "Exit terminal mode" })
map("t", "<C-h>", "<cmd>wincmd h<cr>", { desc = "Go to left window" })
map("t", "<C-j>", "<cmd>wincmd j<cr>", { desc = "Go to lower window" })
map("t", "<C-k>", "<cmd>wincmd k<cr>", { desc = "Go to upper window" })
map("t", "<C-l>", "<cmd>wincmd l<cr>", { desc = "Go to right window" })

-- ── Hex editor mode (cybersec) ────────────────────────────────────────────
map("n", "<leader>xh", ":%!xxd<cr>",         { desc = "Hex view" })
map("n", "<leader>xr", ":%!xxd -r<cr>",      { desc = "Hex revert" })
map("n", "<leader>xb", "<cmd>%!base64 -d<cr>", { desc = "Base64 decode buffer" })
