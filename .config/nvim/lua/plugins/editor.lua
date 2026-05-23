-- =============================================================================
-- Editor utilities -- file tree, fuzzy finder, git, diagnostics, etc.
-- =============================================================================
return {
  -- ── Telescope ───────────────────────────────────────────────────────────
  {
    "nvim-telescope/telescope.nvim",
    branch = "0.1.x",
    cmd    = "Telescope",
    dependencies = {
      "nvim-lua/plenary.nvim",
      { "nvim-telescope/telescope-fzf-native.nvim", build = "make" },
      "nvim-telescope/telescope-ui-select.nvim",
    },
    keys = {
      { "<leader>ff", "<cmd>Telescope find_files<cr>",  desc = "Find files" },
      { "<leader>fg", "<cmd>Telescope live_grep<cr>",   desc = "Live grep" },
      { "<leader>fb", "<cmd>Telescope buffers<cr>",     desc = "Buffers" },
      { "<leader>fh", "<cmd>Telescope help_tags<cr>",   desc = "Help tags" },
      { "<leader>fr", "<cmd>Telescope oldfiles<cr>",    desc = "Recent files" },
      { "<leader>fd", "<cmd>Telescope diagnostics<cr>", desc = "Diagnostics" },
      { "<leader>fc", "<cmd>Telescope commands<cr>",    desc = "Commands" },
      { "<leader>fk", "<cmd>Telescope keymaps<cr>",     desc = "Keymaps" },
      { "<leader>fw", "<cmd>Telescope grep_string<cr>", desc = "Grep word" },
      { "<leader>gs", "<cmd>Telescope git_status<cr>",  desc = "Git status" },
      { "<leader>gc", "<cmd>Telescope git_commits<cr>", desc = "Git commits" },
      { "<leader>/",  "<cmd>Telescope current_buffer_fuzzy_find<cr>", desc = "Fuzzy search buffer" },
    },
    config = function()
      local telescope = require("telescope")
      telescope.setup({
        defaults = {
          prompt_prefix   = "   ",
          selection_caret = "  ",
          sorting_strategy  = "ascending",
          layout_strategy   = "horizontal",
          layout_config     = { prompt_position = "top", width = 0.87, height = 0.80 },
          file_ignore_patterns = {
            "node_modules", ".git/", "__pycache__", "%.o", "%.a", "%.out", "%.class",
            "%.pdf", "%.mkv", "%.mp4", "%.zip",
          },
          mappings = {
            i = {
              ["<C-j>"] = "move_selection_next",
              ["<C-k>"] = "move_selection_previous",
            },
          },
        },
        extensions = {
          fzf = { fuzzy = true, override_generic_sorter = true, override_file_sorter = true },
          ["ui-select"] = { require("telescope.themes").get_dropdown() },
        },
      })
      telescope.load_extension("fzf")
      telescope.load_extension("ui-select")
    end,
  },

  -- ── Neo-tree (file explorer) ────────────────────────────────────────────
  {
    "nvim-neo-tree/neo-tree.nvim",
    branch = "v3.x",
    cmd    = "Neotree",
    dependencies = { "nvim-lua/plenary.nvim", "nvim-tree/nvim-web-devicons", "MunifTanjim/nui.nvim" },
    keys = {
      { "<leader>t", "<cmd>Neotree toggle<cr>", desc = "Toggle file tree" },
    },
    opts = {
      close_if_last_window = true,
      filesystem = {
        follow_current_file  = { enabled = true },
        use_libuv_file_watcher = true,
        filtered_items = {
          visible        = true,
          hide_dotfiles  = false,
          hide_gitignored = false,
          hide_by_name   = { ".git", "node_modules", "__pycache__" },
        },
      },
      window = { width = 32, mappings = { ["<space>"] = "none" } },
      default_component_configs = {
        indent = { with_expanders = true },
      },
    },
  },

  -- ── Gitsigns ────────────────────────────────────────────────────────────
  {
    "lewis6991/gitsigns.nvim",
    event = { "BufReadPre", "BufNewFile" },
    opts = {
      signs = {
        add          = { text = "+" },
        change       = { text = "~" },
        delete       = { text = "_" },
        topdelete    = { text = "^" },
        changedelete = { text = "~" },
      },
      on_attach = function(bufnr)
        local gs  = package.loaded.gitsigns
        local map = function(mode, l, r, desc)
          vim.keymap.set(mode, l, r, { buffer = bufnr, desc = desc })
        end
        map("n", "]h", gs.next_hunk,     "Next git hunk")
        map("n", "[h", gs.prev_hunk,     "Prev git hunk")
        map("n", "<leader>hs", gs.stage_hunk,      "Stage hunk")
        map("n", "<leader>hr", gs.reset_hunk,      "Reset hunk")
        map("n", "<leader>hp", gs.preview_hunk,    "Preview hunk")
        map("n", "<leader>hb", function() gs.blame_line({ full = true }) end, "Blame line")
      end,
    },
  },

  -- ── Trouble (diagnostics panel) ─────────────────────────────────────────
  {
    "folke/trouble.nvim",
    cmd = "Trouble",
    keys = {
      { "<leader>xx", "<cmd>Trouble diagnostics toggle<cr>",              desc = "Diagnostics (Trouble)" },
      { "<leader>xX", "<cmd>Trouble diagnostics toggle filter.buf=0<cr>", desc = "Buffer diagnostics" },
      { "<leader>xl", "<cmd>Trouble loclist toggle<cr>",                  desc = "Location list" },
      { "<leader>xq", "<cmd>Trouble qflist toggle<cr>",                   desc = "Quickfix list" },
    },
    opts = {},
  },

  -- ── Which-key ───────────────────────────────────────────────────────────
  {
    "folke/which-key.nvim",
    event = "VeryLazy",
    opts = {
      plugins  = { spelling = { enabled = true } },
      defaults = {},
    },
    config = function(_, opts)
      local wk = require("which-key")
      wk.setup(opts)
      wk.add({
        { "<leader>b", group = "buffer" },
        { "<leader>c", group = "code" },
        { "<leader>f", group = "find" },
        { "<leader>g", group = "git" },
        { "<leader>h", group = "hunks" },
        { "<leader>r", group = "rename" },
        { "<leader>x", group = "diagnostics/hex" },
      })
    end,
  },

  -- ── Autopairs ───────────────────────────────────────────────────────────
  {
    "windwp/nvim-autopairs",
    event = "InsertEnter",
    dependencies = { "hrsh7th/nvim-cmp" },
    config = function()
      require("nvim-autopairs").setup({ check_ts = true })
      -- Hook into cmp
      local cmp_autopairs = require("nvim-autopairs.completion.cmp")
      require("cmp").event:on("confirm_done", cmp_autopairs.on_confirm_done())
    end,
  },

  -- ── Comment.nvim ────────────────────────────────────────────────────────
  {
    "numToStr/Comment.nvim",
    event = { "BufReadPost", "BufNewFile" },
    opts  = {},
  },

  -- ── Todo comments ──────────────────────────────────────────────────────
  {
    "folke/todo-comments.nvim",
    event = { "BufReadPost", "BufNewFile" },
    dependencies = { "nvim-lua/plenary.nvim" },
    opts  = {},
    keys  = {
      { "<leader>ft", "<cmd>TodoTelescope<cr>", desc = "Find TODOs" },
    },
  },

  -- ── Surround ────────────────────────────────────────────────────────────
  {
    "kylechui/nvim-surround",
    event = { "BufReadPost", "BufNewFile" },
    opts  = {},
  },

  -- ── Indent guides ──────────────────────────────────────────────────────
  {
    "lukas-reineke/indent-blankline.nvim",
    main  = "ibl",
    event = { "BufReadPost", "BufNewFile" },
    opts  = {
      indent  = { char = "│" },
      scope   = { enabled = true, show_start = false, show_end = false },
      exclude = { filetypes = { "help", "dashboard", "neo-tree", "Trouble", "lazy", "mason" } },
    },
  },
}
