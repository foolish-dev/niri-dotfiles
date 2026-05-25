-- =============================================================================
-- AI -- opencode, Copilot, Avante, CodeCompanion
-- =============================================================================
return {
  -- ── opencode.nvim: built-in Teleia-companion popup ─────────────────────
  {
    "NickvanDyke/opencode.nvim",
    version = "*",
    init = function()
      ---@type opencode.Opts
      vim.g.opencode_opts = {}
      vim.o.autoread = true
    end,
    keys = {
      { "<leader>oa", function() require("opencode").ask("@this: ", { submit = true }) end, mode = { "n", "x" }, desc = "Ask opencode" },
      { "<leader>os", function() require("opencode").select() end,                          mode = { "n", "x" }, desc = "Select opencode action" },
      { "<leader>ot", function() require("opencode").toggle() end,                          mode = { "n", "t" }, desc = "Toggle opencode" },
      { "<leader>ou", function() require("opencode").command("session.half.page.up") end,   desc = "Opencode scroll up" },
      { "<leader>od", function() require("opencode").command("session.half.page.down") end, desc = "Opencode scroll down" },
      { "go",  function() return require("opencode").operator("@this ") end,        mode = { "n", "x" }, expr = true, desc = "Add range to opencode" },
      { "goo", function() return require("opencode").operator("@this ") .. "_" end, expr = true,                       desc = "Add line to opencode" },
    },
  },

  -- ── GitHub Copilot: inline ghost-text completions ──────────────────────
  {
    "zbirenbaum/copilot.lua",
    cmd   = "Copilot",
    event = "InsertEnter",
    opts = {
      suggestion = {
        enabled       = true,
        auto_trigger  = true,
        keymap = {
          accept      = "<M-l>",
          accept_word = "<M-w>",
          accept_line = "<M-j>",
          next        = "<M-]>",
          prev        = "<M-[>",
          dismiss     = "<C-]>",
        },
      },
      panel = { enabled = false },
      filetypes = {
        markdown = true, gitcommit = true, ["."] = false,
      },
    },
  },

  -- ── copilot-cmp: surface Copilot completions through nvim-cmp ─────────
  {
    "zbirenbaum/copilot-cmp",
    dependencies = { "zbirenbaum/copilot.lua" },
    event = "InsertEnter",
    config = function() require("copilot_cmp").setup() end,
  },

  -- ── Avante: Cursor-style inline AI assist ──────────────────────────────
  {
    "yetone/avante.nvim",
    event = "VeryLazy",
    version = false,
    opts = {
      provider = "claude",
      claude = {
        endpoint = "https://api.anthropic.com",
        model    = "claude-sonnet-4-6",
        timeout  = 30000,
        temperature = 0,
        max_tokens  = 8192,
      },
      behaviour = {
        auto_suggestions               = false,
        auto_set_highlight_group       = true,
        auto_set_keymaps               = true,
        auto_apply_diff_after_generation = false,
        support_paste_from_clipboard   = true,
      },
    },
    build = "make",
    dependencies = {
      "stevearc/dressing.nvim",
      "nvim-lua/plenary.nvim",
      "MunifTanjim/nui.nvim",
      "hrsh7th/nvim-cmp",
      "nvim-tree/nvim-web-devicons",
      "zbirenbaum/copilot.lua",
      {
        "HakonHarnes/img-clip.nvim",
        event = "VeryLazy",
        opts  = {
          default = { embed_image_as_base64 = false, prompt_for_file_name = false,
                      drag_and_drop = { insert_mode = true }, use_absolute_path = true },
        },
      },
      { "MeanderingProgrammer/render-markdown.nvim", opts = { file_types = { "markdown", "Avante" } }, ft = { "markdown", "Avante" } },
    },
  },

  -- ── CodeCompanion: chat UI + inline actions, BYO model ────────────────
  {
    "olimorris/codecompanion.nvim",
    cmd = { "CodeCompanion", "CodeCompanionChat", "CodeCompanionActions" },
    keys = {
      { "<leader>aa", "<cmd>CodeCompanionActions<cr>",         mode = { "n", "v" }, desc = "CodeCompanion actions" },
      { "<leader>ac", "<cmd>CodeCompanionChat Toggle<cr>",      mode = { "n", "v" }, desc = "CodeCompanion chat" },
      { "<leader>ai", "<cmd>CodeCompanion<cr>",                  mode = { "n", "v" }, desc = "CodeCompanion inline" },
    },
    dependencies = { "nvim-lua/plenary.nvim", "nvim-treesitter/nvim-treesitter" },
    opts = {
      strategies = {
        chat   = { adapter = "anthropic" },
        inline = { adapter = "anthropic" },
      },
    },
  },
}
