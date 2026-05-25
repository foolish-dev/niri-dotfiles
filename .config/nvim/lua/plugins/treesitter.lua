-- =============================================================================
-- Treesitter -- Syntax highlighting, indentation, folding, text objects, ...
-- =============================================================================
return {
  {
    "nvim-treesitter/nvim-treesitter",
    branch = "master", -- legacy API; `main` is the in-progress rewrite
    build  = ":TSUpdate",
    event  = { "BufReadPost", "BufNewFile" },
    dependencies = {
      { "nvim-treesitter/nvim-treesitter-textobjects", branch = "master" },
      "JoosepAlviste/nvim-ts-context-commentstring",
      "windwp/nvim-ts-autotag",
    },
    opts = {
      ensure_installed = {
        -- Systems / security
        "bash", "c", "cpp", "go", "rust", "zig", "asm", "cuda", "make",
        -- Scripting
        "python", "lua", "luadoc", "luap", "ruby", "perl",
        -- Web
        "javascript", "typescript", "tsx", "html", "css", "scss",
        "json", "jsonc", "json5", "vue", "svelte", "astro", "prisma",
        "graphql", "styled",
        -- Config / IaC
        "yaml", "toml", "kdl", "nix", "dockerfile", "terraform", "hcl",
        "ini", "ssh_config", "passwd", "gitignore", "gitattributes",
        -- Data / docs
        "markdown", "markdown_inline", "regex", "sql", "xml", "csv", "rst",
        -- Functional / other
        "haskell", "elixir", "erlang", "ocaml", "ocaml_interface",
        "kotlin", "java", "scala", "elm", "fennel", "clojure",
        "julia", "r", "fortran", "racket", "scheme",
        -- Cybersec-adjacent
        "bitbake", "tcl",
        -- Misc
        "vim", "vimdoc", "query", "diff", "gitcommit", "git_rebase",
        "git_config", "comment", "todotxt", "jq", "awk", "printf", "po",
      },
      sync_install = false,
      auto_install = true,
      highlight    = { enable = true, additional_vim_regex_highlighting = false },
      indent       = { enable = true },
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
            ["aC"] = "@comment.outer",
            ["iC"] = "@comment.outer",
          },
        },
        move = {
          enable    = true,
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

  -- ── ts-autotag: auto-close & rename HTML/JSX tags ──────────────────────
  {
    "windwp/nvim-ts-autotag",
    event = { "BufReadPost", "BufNewFile" },
    opts  = {},
  },

  -- ── ts-context-commentstring: language-aware commentstring ─────────────
  {
    "JoosepAlviste/nvim-ts-context-commentstring",
    lazy = true,
    opts = { enable_autocmd = false },
  },

  -- ── treesitter-context: sticky scope at the top of the buffer ──────────
  {
    "nvim-treesitter/nvim-treesitter-context",
    event = { "BufReadPost", "BufNewFile" },
    keys  = {
      { "<leader>cc", function() require("treesitter-context").go_to_context(vim.v.count1) end, desc = "Go to context" },
    },
    opts = { max_lines = 4, multiline_threshold = 4, mode = "cursor" },
  },

  -- ── rainbow-delimiters: rainbow brackets via treesitter ────────────────
  {
    "HiPhish/rainbow-delimiters.nvim",
    event = { "BufReadPost", "BufNewFile" },
    config = function()
      require("rainbow-delimiters.setup").setup({})
    end,
  },
}
