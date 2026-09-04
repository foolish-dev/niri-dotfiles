-- =============================================================================
-- Coding extras -- DAP, testing, terminal, git client, lang-specific tools
-- =============================================================================
return {
  -- ── Toggleterm (floating / split terminals) ─────────────────────────────
  {
    "akinsho/toggleterm.nvim",
    version = "*",
    keys = {
      { "<leader>th", "<cmd>ToggleTerm direction=horizontal size=15<cr>", desc = "Terminal (horizontal)" },
      { "<leader>tv", "<cmd>ToggleTerm direction=vertical   size=80<cr>", desc = "Terminal (vertical)" },
      { "<leader>tf", "<cmd>ToggleTerm direction=float<cr>",              desc = "Terminal (float)" },
      { "<C-\\>",     "<cmd>ToggleTerm direction=float<cr>",              desc = "Toggle float terminal" },
    },
    opts = {
      shade_terminals = true,
      shading_factor  = -10,
      float_opts      = { border = "rounded", winblend = 8 },
    },
  },

  -- ── DAP (Debug Adapter Protocol) ────────────────────────────────────────
  {
    "mfussenegger/nvim-dap",
    dependencies = {
      "rcarriga/nvim-dap-ui",
      "nvim-neotest/nvim-nio",
      "theHamsta/nvim-dap-virtual-text",
      "mfussenegger/nvim-dap-python",
      "leoluz/nvim-dap-go",
      "jay-babu/mason-nvim-dap.nvim",
    },
    keys = {
      { "<leader>db", function() require("dap").toggle_breakpoint() end, desc = "Toggle breakpoint" },
      { "<leader>dB", function() require("dap").set_breakpoint(vim.fn.input("Condition: ")) end, desc = "Conditional breakpoint" },
      { "<leader>dc", function() require("dap").continue() end,    desc = "Continue" },
      { "<leader>do", function() require("dap").step_over() end,   desc = "Step over" },
      { "<leader>di", function() require("dap").step_into() end,   desc = "Step into" },
      { "<leader>dO", function() require("dap").step_out() end,    desc = "Step out" },
      { "<leader>dr", function() require("dap").repl.toggle() end, desc = "Toggle REPL" },
      { "<leader>du", function() require("dapui").toggle() end,    desc = "Toggle DAP UI" },
      { "<leader>dx", function() require("dap").terminate() end,   desc = "Terminate" },
      { "<leader>dl", function() require("dap").run_last() end,    desc = "Run last" },
    },
    config = function()
      local dap   = require("dap")
      local dapui = require("dapui")

      dapui.setup({
        layouts = {
          { elements = { "scopes", "breakpoints", "stacks", "watches" }, size = 40,   position = "left" },
          { elements = { "repl", "console" },                            size = 0.25, position = "bottom" },
        },
      })
      require("nvim-dap-virtual-text").setup()

      dap.listeners.after.event_initialized["dapui_config"]  = function() dapui.open() end
      dap.listeners.before.event_terminated["dapui_config"]  = function() dapui.close() end
      dap.listeners.before.event_exited["dapui_config"]      = function() dapui.close() end

      -- Python (debugpy via mason)
      pcall(function() require("dap-python").setup("python3") end)
      -- Go (dlv via mason)
      pcall(function() require("dap-go").setup() end)

      -- C / C++ / Rust via codelldb (mason). Prefer $PATH, fall back to
      -- mason's bin dir so DAP works before $PATH is reloaded.
      local codelldb = vim.fn.exepath("codelldb")
      if codelldb == "" then
        codelldb = vim.fn.stdpath("data") .. "/mason/bin/codelldb"
      end
      dap.adapters.codelldb = {
        type = "server", port = "${port}",
        executable = { command = codelldb, args = { "--port", "${port}" } },
      }
      local lldb_cfg = {
        {
          name    = "Launch (codelldb)",
          type    = "codelldb",
          request = "launch",
          program = function() return vim.fn.input("Path to executable: ", vim.fn.getcwd() .. "/", "file") end,
          cwd     = "${workspaceFolder}",
          stopOnEntry = false,
        },
      }
      dap.configurations.c    = lldb_cfg
      dap.configurations.cpp  = lldb_cfg
      dap.configurations.rust = lldb_cfg

      -- Mason-managed DAP adapters. `automatic_installation = true` and
      -- `ensure_installed` race on the same packages at startup; rely on
      -- ensure_installed alone.
      require("mason-nvim-dap").setup({
        ensure_installed = { "python", "delve", "codelldb", "js" },
        automatic_installation = false,
        handlers = {},
      })
    end,
  },

  -- ── neotest: runner integration ─────────────────────────────────────────
  {
    "nvim-neotest/neotest",
    dependencies = {
      "nvim-lua/plenary.nvim",
      "antoinemadec/FixCursorHold.nvim",
      "nvim-treesitter/nvim-treesitter",
      "nvim-neotest/neotest-python",
      "nvim-neotest/neotest-go",
      -- rustaceanvim ships the rust adapter and its docs say not to add
      -- neotest-rust alongside it; rouge8/neotest-rust is archived read-only.
      "mrcjkb/rustaceanvim",
      "nvim-neotest/neotest-jest",
      "marilari88/neotest-vitest",
      "nvim-neotest/neotest-plenary",
    },
    keys = {
      { "<leader>nt", function() require("neotest").run.run() end,                          desc = "Run nearest test" },
      { "<leader>nT", function() require("neotest").run.run(vim.fn.expand("%")) end,         desc = "Run file tests" },
      { "<leader>na", function() require("neotest").run.run(vim.uv.cwd()) end,               desc = "Run all tests" },
      { "<leader>ns", function() require("neotest").summary.toggle() end,                    desc = "Toggle summary" },
      { "<leader>no", function() require("neotest").output.open({ enter = true }) end,        desc = "Test output" },
      { "<leader>nO", function() require("neotest").output_panel.toggle() end,                desc = "Output panel" },
      { "<leader>nx", function() require("neotest").run.stop() end,                          desc = "Stop test" },
    },
    config = function()
      require("neotest").setup({
        adapters = {
          require("neotest-python")({ runner = "pytest" }),
          require("neotest-go"),
          require("rustaceanvim.neotest"),
          require("neotest-jest"),
          require("neotest-vitest"),
          require("neotest-plenary"),
        },
      })
    end,
  },

  -- ── Fugitive (git) ─────────────────────────────────────────────────────
  {
    "tpope/vim-fugitive",
    cmd  = { "Git", "G", "Gdiffsplit", "Gvdiffsplit" },
    keys = {
      { "<leader>gg", "<cmd>Git<cr>",        desc = "Git status (fugitive)" },
      { "<leader>gd", "<cmd>Gdiffsplit<cr>", desc = "Git diff split" },
    },
  },

  -- ── lazygit.nvim: lazygit floating popup ───────────────────────────────
  {
    "kdheepak/lazygit.nvim",
    cmd  = { "LazyGit", "LazyGitConfig", "LazyGitFilterCurrentFile", "LazyGitFilter" },
    keys = { { "<leader>gG", "<cmd>LazyGit<cr>", desc = "Lazygit" } },
    dependencies = { "nvim-lua/plenary.nvim" },
  },

  -- ── Diffview ────────────────────────────────────────────────────────────
  {
    "sindrets/diffview.nvim",
    cmd  = { "DiffviewOpen", "DiffviewFileHistory" },
    keys = {
      { "<leader>gv", "<cmd>DiffviewOpen<cr>",           desc = "Diffview open" },
      { "<leader>gh", "<cmd>DiffviewFileHistory %<cr>",  desc = "File history" },
    },
    opts = {},
  },

  -- ── Markdown preview ───────────────────────────────────────────────────
  {
    "iamcco/markdown-preview.nvim",
    build = "cd app && npx --yes yarn install",
    ft    = { "markdown" },
    cmd   = { "MarkdownPreview", "MarkdownPreviewToggle" },
    keys  = {
      { "<leader>mp", "<cmd>MarkdownPreviewToggle<cr>", desc = "Markdown preview (browser)" },
    },
  },

  -- ── render-markdown.nvim: pretty markdown rendering in the buffer ──────
  {
    "MeanderingProgrammer/render-markdown.nvim",
    ft = { "markdown", "Avante" },
    dependencies = { "nvim-treesitter/nvim-treesitter", "nvim-tree/nvim-web-devicons" },
    opts = {
      file_types = { "markdown", "Avante" },
      heading    = { enabled = true, position = "inline" },
      code       = { enabled = true, sign = false, width = "block", min_width = 60 },
    },
  },

  -- ── Hex editing (cybersec) ─────────────────────────────────────────────
  {
    "RaafatTurki/hex.nvim",
    cmd  = { "HexDump", "HexAssemble", "HexToggle" },
    keys = { { "<leader>xH", "<cmd>HexToggle<cr>", desc = "Toggle hex editor" } },
    opts = {},
  },

  -- ── HTTP client (API testing) ──────────────────────────────────────────
  -- kulala.nvim: pure-Lua successor to rest.nvim; no luarocks bootstrap.
  {
    "mistweaverco/kulala.nvim",
    ft   = { "http", "rest" },
    keys = {
      { "<leader>rr", function() require("kulala").run() end,     ft = { "http", "rest" }, desc = "Run HTTP request" },
      { "<leader>rl", function() require("kulala").replay() end,  ft = { "http", "rest" }, desc = "Replay last request" },
      { "<leader>ra", function() require("kulala").run_all() end, ft = { "http", "rest" }, desc = "Run all requests in buffer" },
    },
    opts = {
      global_keymaps = false,
    },
  },

  -- ── rustaceanvim: Rust LSP + integrations ──────────────────────────────
  {
    "mrcjkb/rustaceanvim",
    version = "^9",
    ft = { "rust" },
    init = function()
      vim.g.rustaceanvim = {
        server = { default_settings = {
          ["rust-analyzer"] = {
            cargo       = { allFeatures = true },
            checkOnSave = true,
            check       = { command = "clippy" },
            inlayHints  = { closingBraceHints = { enable = true }, parameterHints = { enable = true } },
          },
        } },
      }
    end,
  },

  -- ── crates.nvim: Cargo.toml inline crate info ──────────────────────────
  {
    "saecki/crates.nvim",
    event = { "BufRead Cargo.toml" },
    opts  = { completion = { cmp = { enabled = true } } },
  },

  -- ── go.nvim: Go productivity wrapper ───────────────────────────────────
  {
    "ray-x/go.nvim",
    dependencies = { "ray-x/guihua.lua", "neovim/nvim-lspconfig", "nvim-treesitter/nvim-treesitter" },
    ft = { "go", "gomod", "gosum", "gowork" },
    build = function() require("go.install").update_all_sync() end,
    opts = { lsp_cfg = false }, -- nvim-lspconfig already handles gopls
    config = function(_, opts) require("go").setup(opts) end,
  },

  -- ── typescript-tools.nvim: faster ts_ls alternative ────────────────────
  {
    "pmizio/typescript-tools.nvim",
    ft = { "typescript", "typescriptreact", "javascript", "javascriptreact" },
    dependencies = { "nvim-lua/plenary.nvim", "neovim/nvim-lspconfig" },
    opts = {},
  },

  -- ── venv-selector: pick a Python venv per project ──────────────────────
  {
    "linux-cultist/venv-selector.nvim",
    branch = "regexp",
    dependencies = { "neovim/nvim-lspconfig", "nvim-telescope/telescope.nvim", "mfussenegger/nvim-dap-python" },
    ft   = "python",
    cmd  = "VenvSelect",
    keys = { { "<leader>cv", "<cmd>VenvSelect<cr>", desc = "Select Python venv" } },
    opts = {},
  },
}
