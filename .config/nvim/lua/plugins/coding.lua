-- =============================================================================
-- Coding extras -- DAP debugger, terminal, git client, markdown
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
      -- Python
      "mfussenegger/nvim-dap-python",
    },
    keys = {
      { "<leader>db", function() require("dap").toggle_breakpoint() end,  desc = "Toggle breakpoint" },
      { "<leader>dc", function() require("dap").continue() end,          desc = "Continue" },
      { "<leader>do", function() require("dap").step_over() end,         desc = "Step over" },
      { "<leader>di", function() require("dap").step_into() end,         desc = "Step into" },
      { "<leader>dO", function() require("dap").step_out() end,          desc = "Step out" },
      { "<leader>dr", function() require("dap").repl.toggle() end,       desc = "Toggle REPL" },
      { "<leader>du", function() require("dapui").toggle() end,          desc = "Toggle DAP UI" },
      { "<leader>dx", function() require("dap").terminate() end,         desc = "Terminate" },
    },
    config = function()
      local dap   = require("dap")
      local dapui = require("dapui")

      dapui.setup({
        layouts = {
          {
            elements = { "scopes", "breakpoints", "stacks", "watches" },
            size = 40, position = "left",
          },
          {
            elements = { "repl", "console" },
            size = 0.25, position = "bottom",
          },
        },
      })
      require("nvim-dap-virtual-text").setup()

      -- Auto open/close dapui
      dap.listeners.after.event_initialized["dapui_config"]  = function() dapui.open()  end
      dap.listeners.before.event_terminated["dapui_config"]  = function() dapui.close() end
      dap.listeners.before.event_exited["dapui_config"]      = function() dapui.close() end

      -- Python adapter
      require("dap-python").setup("python3")

      -- GDB / C / C++ / Rust adapter
      dap.adapters.gdb = {
        type    = "executable",
        command = "gdb",
        args    = { "-i", "dap" },
      }
      dap.configurations.c = {
        {
          name    = "Launch (GDB)",
          type    = "gdb",
          request = "launch",
          program = function()
            return vim.fn.input("Path to executable: ", vim.fn.getcwd() .. "/", "file")
          end,
          cwd = "${workspaceFolder}",
        },
      }
      dap.configurations.cpp  = dap.configurations.c
      dap.configurations.rust = dap.configurations.c
    end,
  },

  -- ── Fugitive (git) ─────────────────────────────────────────────────────
  {
    "tpope/vim-fugitive",
    cmd  = { "Git", "G", "Gdiffsplit", "Gvdiffsplit" },
    keys = {
      { "<leader>gg", "<cmd>Git<cr>",      desc = "Git status (fugitive)" },
      { "<leader>gd", "<cmd>Gdiffsplit<cr>", desc = "Git diff split" },
    },
  },

  -- ── Diffview ────────────────────────────────────────────────────────────
  {
    "sindrets/diffview.nvim",
    cmd  = { "DiffviewOpen", "DiffviewFileHistory" },
    keys = {
      { "<leader>gv", "<cmd>DiffviewOpen<cr>",          desc = "Diffview open" },
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
      { "<leader>mp", "<cmd>MarkdownPreviewToggle<cr>", desc = "Markdown preview" },
    },
  },

  -- ── Hex editing (cybersec) ─────────────────────────────────────────────
  {
    "RaafatTurki/hex.nvim",
    cmd  = { "HexDump", "HexAssemble", "HexToggle" },
    keys = {
      { "<leader>xH", "<cmd>HexToggle<cr>", desc = "Toggle hex editor" },
    },
    opts = {},
  },

  -- ── Rest client (API testing) ──────────────────────────────────────────
  {
    "rest-nvim/rest.nvim",
    ft   = "http",
    dependencies = { "nvim-neotest/nvim-nio" },
    keys = {
      { "<leader>rr", "<cmd>Rest run<cr>",     desc = "Run HTTP request" },
      { "<leader>rl", "<cmd>Rest run last<cr>", desc = "Re-run last request" },
    },
    -- rest.nvim v2+ uses a completely different config schema; the minimal
    -- default opts work out of the box. Run :Rest env show to manage
    -- environment files; :Rest run / :Rest last for requests.
    opts = {},
  },
}
