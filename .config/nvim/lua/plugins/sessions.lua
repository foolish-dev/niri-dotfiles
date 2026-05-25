-- =============================================================================
-- Sessions & projects -- persistence, project detection
-- =============================================================================
return {
  -- ── persistence.nvim: auto-save / restore the last session per cwd ────
  {
    "folke/persistence.nvim",
    event = "BufReadPre",
    opts  = { options = { "buffers", "curdir", "tabpages", "winsize", "help", "globals" } },
    keys  = {
      { "<leader>ss", function() require("persistence").load() end,                       desc = "Restore session" },
      { "<leader>sl", function() require("persistence").load({ last = true }) end,        desc = "Restore last session" },
      { "<leader>sd", function() require("persistence").stop() end,                        desc = "Stop session save" },
    },
  },

  -- ── project.nvim: detect project root + telescope picker ──────────────
  {
    "ahmedkhalf/project.nvim",
    event = "VeryLazy",
    config = function()
      -- pattern-only detection: project.nvim hasn't been updated since 2023
      -- and its `lsp` detection path calls vim.lsp.buf_get_clients(), which
      -- is deprecated and slated for removal in nvim 0.13. The patterns
      -- below cover the Rust/Python/Go/Lua/JS stack already.
      require("project_nvim").setup({
        detection_methods = { "pattern" },
        patterns          = { ".git", "Cargo.toml", "package.json", "pyproject.toml", "go.mod", "Makefile" },
      })
      pcall(function() require("telescope").load_extension("projects") end)
    end,
    keys = {
      { "<leader>fp", "<cmd>Telescope projects<cr>", desc = "Projects" },
    },
  },
}
