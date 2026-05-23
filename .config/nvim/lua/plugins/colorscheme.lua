-- =============================================================================
-- Colorscheme -- Tokyo Night (night variant, transparent)
-- =============================================================================
return {
  {
    "folke/tokyonight.nvim",
    lazy = false,
    priority = 1000,
    opts = {
      style = "night",
      transparent = true,
      terminal_colors = true,
      styles = {
        comments  = { italic = true },
        keywords  = { italic = true },
        functions = {},
        variables = {},
        sidebars  = "transparent",
        floats    = "dark",
      },
      sidebars = { "qf", "help", "terminal", "lazy", "neo-tree", "Trouble" },
      on_highlights = function(hl, c)
        hl.CursorLineNr = { fg = c.orange, bold = true }
        hl.LineNr        = { fg = c.dark3 }
        hl.DiagnosticVirtualTextError = { bg = c.none, fg = c.error }
        hl.DiagnosticVirtualTextWarn  = { bg = c.none, fg = c.warning }
        hl.DiagnosticVirtualTextInfo  = { bg = c.none, fg = c.info }
        hl.DiagnosticVirtualTextHint  = { bg = c.none, fg = c.hint }
      end,
    },
    config = function(_, opts)
      require("tokyonight").setup(opts)
      vim.cmd.colorscheme("grogu")
    end,
  },
}
