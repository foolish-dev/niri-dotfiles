-- =============================================================================
-- Treesitter -- Syntax highlighting, indentation, folding
-- =============================================================================
return {
  {
    "nvim-treesitter/nvim-treesitter",
    build = ":TSUpdate",
    event = { "BufReadPost", "BufNewFile" },
    dependencies = {
      "nvim-treesitter/nvim-treesitter-textobjects",
    },
    opts = {
      ensure_installed = {
        -- Systems / security
        "bash", "c", "cpp", "go", "rust", "zig", "asm",
        -- Scripting
        "python", "lua", "ruby", "perl",
        -- Web
        "javascript", "typescript", "tsx", "html", "css", "json", "jsonc",
        -- Config / IaC
        "yaml", "toml", "kdl", "nix", "dockerfile", "terraform", "hcl",
        -- Data / docs
        "markdown", "markdown_inline", "regex", "sql", "graphql", "xml",
        -- Misc
        "vim", "vimdoc", "query", "diff", "gitcommit", "git_rebase",
      },
      highlight = { enable = true },
      indent    = { enable = true },
      incremental_selection = {
        enable  = true,
        keymaps = {
          init_selection    = "<C-space>",
          node_incremental  = "<C-space>",
          scope_incremental = false,
          node_decremental  = "<bs>",
        },
      },
      textobjects = {
        select = {
          enable    = true,
          lookahead = true,
          keymaps   = {
            ["af"] = "@function.outer",
            ["if"] = "@function.inner",
            ["ac"] = "@class.outer",
            ["ic"] = "@class.inner",
            ["aa"] = "@parameter.outer",
            ["ia"] = "@parameter.inner",
            ["ai"] = "@conditional.outer",
            ["ii"] = "@conditional.inner",
            ["al"] = "@loop.outer",
            ["il"] = "@loop.inner",
          },
        },
        move = {
          enable = true,
          set_jumps = true,
          goto_next_start     = { ["]f"] = "@function.outer", ["]c"] = "@class.outer" },
          goto_next_end       = { ["]F"] = "@function.outer", ["]C"] = "@class.outer" },
          goto_previous_start = { ["[f"] = "@function.outer", ["[c"] = "@class.outer" },
          goto_previous_end   = { ["[F"] = "@function.outer", ["[C"] = "@class.outer" },
        },
        swap = {
          enable = true,
          swap_next     = { ["<leader>a"] = "@parameter.inner" },
          swap_previous = { ["<leader>A"] = "@parameter.inner" },
        },
      },
    },
    config = function(_, opts)
      require("nvim-treesitter.configs").setup(opts)
    end,
  },
}
