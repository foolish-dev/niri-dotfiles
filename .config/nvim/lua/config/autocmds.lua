-- =============================================================================
-- Autocommands
-- =============================================================================
local autocmd = vim.api.nvim_create_autocmd
local augroup = vim.api.nvim_create_augroup

-- ── Highlight on yank ─────────────────────────────────────────────────────
autocmd("TextYankPost", {
  group = augroup("YankHighlight", { clear = true }),
  callback = function()
    (vim.hl or vim.highlight).on_yank({ higroup = "IncSearch", timeout = 200 })
  end,
})

-- ── Restore cursor position ───────────────────────────────────────────────
autocmd("BufReadPost", {
  group = augroup("RestoreCursor", { clear = true }),
  callback = function(ev)
    local mark = vim.api.nvim_buf_get_mark(ev.buf, '"')
    local lcount = vim.api.nvim_buf_line_count(ev.buf)
    if mark[1] > 0 and mark[1] <= lcount then
      pcall(vim.api.nvim_win_set_cursor, 0, mark)
    end
  end,
})

-- ── Auto-resize splits on terminal resize ─────────────────────────────────
autocmd("VimResized", {
  group = augroup("AutoResize", { clear = true }),
  callback = function() vim.cmd("tabdo wincmd =") end,
})

-- ── Close specific buffers with q ─────────────────────────────────────────
autocmd("FileType", {
  group = augroup("CloseWithQ", { clear = true }),
  pattern = { "help", "man", "qf", "lspinfo", "startuptime", "checkhealth" },
  callback = function(ev)
    vim.bo[ev.buf].buflisted = false
    vim.keymap.set("n", "q", "<cmd>close<cr>", { buffer = ev.buf, silent = true })
  end,
})

-- ── Filetype overrides ───────────────────────────────────────────────────
autocmd("FileType", {
  group = augroup("IndentOverrides", { clear = true }),
  pattern = { "lua", "nix", "yaml", "json", "html", "css", "javascript", "typescript", "typescriptreact" },
  callback = function()
    vim.opt_local.shiftwidth  = 2
    vim.opt_local.tabstop     = 2
    vim.opt_local.softtabstop = 2
  end,
})

-- ── Recognize security-related filetypes ──────────────────────────────────
autocmd({ "BufRead", "BufNewFile" }, {
  group = augroup("SecurityFiletypes", { clear = true }),
  pattern = { "*.nse", "*.rules" },
  callback = function(ev)
    if ev.match:match("%.nse$") then
      vim.bo[ev.buf].filetype = "lua"
    elseif ev.match:match("%.rules$") then
      vim.bo[ev.buf].filetype = "conf"
    end
  end,
})

-- ── Auto-open Neo-tree on startup (opencode is a lazy plugin) ─────────────
autocmd("VimEnter", {
  group = augroup("AutoOpenLayout", { clear = true }),
  callback = function()
    if vim.fn.argc() == 0 or vim.o.diff then return end
    local first = vim.fn.argv(0) or ""
    local skip_patterns = {
      "EDITMSG$",          -- COMMIT_EDITMSG, MERGE_MSG, TAG_EDITMSG, SQUASH_MSG
      "git%-rebase%-todo$",
      "/crontab%.",
      "sudoers%.tmp",
    }
    for _, pat in ipairs(skip_patterns) do
      if first:match(pat) then return end
    end

    local file_win = vim.api.nvim_get_current_win()
    vim.schedule(function()
      vim.cmd("Neotree show")
      if vim.api.nvim_win_is_valid(file_win) then
        vim.api.nvim_set_current_win(file_win)
      end
    end)
  end,
})
