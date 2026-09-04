-- =============================================================================
-- Editor utilities -- file tree, fuzzy finder, git, motion, marks, folding, etc.
-- =============================================================================
return {
  -- ── Telescope ───────────────────────────────────────────────────────────
  {
    "nvim-telescope/telescope.nvim",
    branch = "0.1.x",
    cmd = "Telescope",
    dependencies = {
      "nvim-lua/plenary.nvim",
      { "nvim-telescope/telescope-fzf-native.nvim", build = "make" },
      "nvim-telescope/telescope-ui-select.nvim",
      "nvim-telescope/telescope-file-browser.nvim",
      "nvim-telescope/telescope-live-grep-args.nvim",
    },
    keys = {
      { "<leader>ff", "<cmd>Telescope find_files<cr>",   desc = "Find files" },
      { "<leader>fg", "<cmd>Telescope live_grep<cr>",    desc = "Live grep" },
      { "<leader>fG", "<cmd>Telescope live_grep_args<cr>", desc = "Live grep (args)" },
      { "<leader>fb", "<cmd>Telescope buffers<cr>",      desc = "Buffers" },
      { "<leader>fh", "<cmd>Telescope help_tags<cr>",    desc = "Help tags" },
      { "<leader>fr", "<cmd>Telescope oldfiles<cr>",     desc = "Recent files" },
      { "<leader>fd", "<cmd>Telescope diagnostics<cr>",  desc = "Diagnostics" },
      { "<leader>fc", "<cmd>Telescope commands<cr>",     desc = "Commands" },
      { "<leader>fk", "<cmd>Telescope keymaps<cr>",      desc = "Keymaps" },
      { "<leader>fw", "<cmd>Telescope grep_string<cr>",  desc = "Grep word" },
      { "<leader>fR", "<cmd>Telescope resume<cr>",       desc = "Resume picker" },
      { "<leader>fj", "<cmd>Telescope jumplist<cr>",     desc = "Jumplist" },
      { "<leader>fm", "<cmd>Telescope marks<cr>",        desc = "Marks" },
      { "<leader>fF", "<cmd>Telescope file_browser<cr>", desc = "File browser" },
      { "<leader>gs", "<cmd>Telescope git_status<cr>",   desc = "Git status" },
      { "<leader>gc", "<cmd>Telescope git_commits<cr>",  desc = "Git commits" },
      { "<leader>gB", "<cmd>Telescope git_branches<cr>", desc = "Git branches" },
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
            "%.pdf", "%.mkv", "%.mp4", "%.zip", "target/",
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
      telescope.load_extension("file_browser")
      telescope.load_extension("live_grep_args")
    end,
  },

  -- ── Neo-tree (file explorer) ────────────────────────────────────────────
  {
    "nvim-neo-tree/neo-tree.nvim",
    branch = "v3.x",
    cmd = "Neotree",
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
      default_component_configs = { indent = { with_expanders = true } },
    },
  },

  -- ── oil.nvim: edit the filesystem as a buffer ──────────────────────────
  {
    "stevearc/oil.nvim",
    cmd  = "Oil",
    keys = {
      { "-",          "<cmd>Oil<cr>",       desc = "Open parent dir in Oil" },
      { "<leader>fe", "<cmd>Oil --float<cr>", desc = "Edit dir (Oil float)" },
    },
    opts = {
      default_file_explorer = false,
      view_options = { show_hidden = true },
      float = { padding = 2, max_width = 100, max_height = 24, border = "rounded" },
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
        map("n", "]h", gs.next_hunk, "Next git hunk")
        map("n", "[h", gs.prev_hunk, "Prev git hunk")
        map("n", "<leader>hs", gs.stage_hunk,   "Stage hunk")
        map("n", "<leader>hr", gs.reset_hunk,   "Reset hunk")
        map("n", "<leader>hp", gs.preview_hunk, "Preview hunk")
        map("n", "<leader>hb", function() gs.blame_line({ full = true }) end, "Blame line")
        map("n", "<leader>hd", gs.diffthis, "Diff against index")
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
      { "<leader>xs", "<cmd>Trouble symbols toggle<cr>",                  desc = "Symbols (Trouble)" },
      { "<leader>xL", "<cmd>Trouble lsp toggle<cr>",                      desc = "LSP refs (Trouble)" },
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
        { "<leader>d", group = "debug" },
        { "<leader>f", group = "find" },
        { "<leader>g", group = "git" },
        { "<leader>h", group = "hunks" },
        { "<leader>r", group = "rename / http" },
        { "<leader>t", group = "terminal / tree" },
        { "<leader>n", group = "neotest" },
        { "<leader>o", group = "opencode" },
        { "<leader>x", group = "diagnostics/hex" },
        { "<leader>s", group = "search / session" },
        { "<leader>a", group = "ai" },
        { "<leader>z", group = "zen / focus" },
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
      local cmp_autopairs = require("nvim-autopairs.completion.cmp")
      require("cmp").event:on("confirm_done", cmp_autopairs.on_confirm_done())
    end,
  },

  -- ── Comment.nvim ────────────────────────────────────────────────────────
  {
    "numToStr/Comment.nvim",
    event = { "BufReadPost", "BufNewFile" },
    dependencies = { "JoosepAlviste/nvim-ts-context-commentstring" },
    opts = function()
      local ok, ts = pcall(require, "ts_context_commentstring.integrations.comment_nvim")
      return { pre_hook = ok and ts.create_pre_hook() or nil }
    end,
  },

  -- ── Todo comments ──────────────────────────────────────────────────────
  {
    "folke/todo-comments.nvim",
    event = { "BufReadPost", "BufNewFile" },
    dependencies = { "nvim-lua/plenary.nvim" },
    opts = {},
    keys = {
      { "<leader>ft", "<cmd>TodoTelescope<cr>", desc = "Find TODOs" },
      { "]t", function() require("todo-comments").jump_next() end, desc = "Next todo" },
      { "[t", function() require("todo-comments").jump_prev() end, desc = "Prev todo" },
    },
  },

  -- ── Surround ────────────────────────────────────────────────────────────
  {
    "kylechui/nvim-surround",
    event = { "BufReadPost", "BufNewFile" },
    opts = {},
  },

  -- ── Indent guides ──────────────────────────────────────────────────────
  {
    "lukas-reineke/indent-blankline.nvim",
    main  = "ibl",
    event = { "BufReadPost", "BufNewFile" },
    opts = {
      indent  = { char = "│" },
      scope   = { enabled = true, show_start = false, show_end = false },
      exclude = { filetypes = { "help", "dashboard", "neo-tree", "Trouble", "lazy", "mason" } },
    },
  },

  -- ── flash.nvim: jump motions, treesitter selection, remote ops ─────────
  {
    "folke/flash.nvim",
    event = "VeryLazy",
    opts  = {},
    keys = {
      { "s", function() require("flash").jump() end,         mode = { "n", "x", "o" }, desc = "Flash jump" },
      -- No "x" here: nvim-surround maps x S unconditionally from its own
      -- plugin/ file, and whichever of the two loads last silently wins --
      -- surround after `nvim`, flash after `nvim <file>`, because lazy's
      -- Keys handler deletes the existing mapping when it installs its own.
      -- Visual S is surround's headline mapping; flash keeps R (x, o) there.
      { "S", function() require("flash").treesitter() end,    mode = { "n", "o" },      desc = "Flash treesitter" },
      { "r", function() require("flash").remote() end,        mode = "o",               desc = "Flash remote" },
      { "R", function() require("flash").treesitter_search() end, mode = { "o", "x" },   desc = "Flash ts search" },
    },
  },

  -- ── vim-illuminate: highlight references under cursor ──────────────────
  {
    "RRethy/vim-illuminate",
    event = { "BufReadPost", "BufNewFile" },
    opts  = { providers = { "lsp", "treesitter", "regex" }, delay = 120 },
    config = function(_, opts) require("illuminate").configure(opts) end,
  },

  -- ── nvim-spectre: project-wide search and replace ──────────────────────
  {
    "nvim-pack/nvim-spectre",
    cmd  = "Spectre",
    keys = {
      { "<leader>sr", function() require("spectre").open() end, desc = "Spectre: search/replace" },
      { "<leader>sw", function() require("spectre").open_visual({ select_word = true }) end, desc = "Spectre: word under cursor" },
    },
    opts = {},
  },

  -- ── nvim-ufo: better folding (uses treesitter + LSP) ───────────────────
  {
    "kevinhwang91/nvim-ufo",
    event = { "BufReadPost", "BufNewFile" },
    dependencies = { "kevinhwang91/promise-async" },
    init = function()
      vim.o.foldcolumn     = "1"
      vim.o.foldlevel      = 99
      vim.o.foldlevelstart = 99
      vim.o.foldenable     = true
    end,
    keys = {
      { "zR", function() require("ufo").openAllFolds() end,  desc = "Open all folds" },
      { "zM", function() require("ufo").closeAllFolds() end, desc = "Close all folds" },
    },
    opts = {
      provider_selector = function() return { "treesitter", "indent" } end,
    },
  },

  -- ── harpoon: per-project quick marks ───────────────────────────────────
  {
    "ThePrimeagen/harpoon",
    branch = "harpoon2",
    dependencies = { "nvim-lua/plenary.nvim" },
    keys = {
      { "<leader>ma", function() require("harpoon"):list():add() end,    desc = "Harpoon add" },
      { "<leader>mm", function() local h = require("harpoon") h.ui:toggle_quick_menu(h:list()) end, desc = "Harpoon menu" },
      { "<leader>1",  function() require("harpoon"):list():select(1) end, desc = "Harpoon 1" },
      { "<leader>2",  function() require("harpoon"):list():select(2) end, desc = "Harpoon 2" },
      { "<leader>3",  function() require("harpoon"):list():select(3) end, desc = "Harpoon 3" },
      { "<leader>4",  function() require("harpoon"):list():select(4) end, desc = "Harpoon 4" },
    },
    config = function() require("harpoon"):setup() end,
  },

  -- ── mini.ai: smarter text objects ──────────────────────────────────────
  {
    "echasnovski/mini.ai",
    event = { "BufReadPost", "BufNewFile" },
    opts  = { n_lines = 500 },
  },

  -- ── mini.bracketed: rich [/] navigation ────────────────────────────────
  {
    "echasnovski/mini.bracketed",
    event = "VeryLazy",
    opts  = {},
  },

  -- ── mini.indentscope: animated current-scope indent line ──────────────
  {
    "echasnovski/mini.indentscope",
    event = { "BufReadPost", "BufNewFile" },
    opts  = {
      symbol = "│",
      draw   = { animation = function() return 10 end },
    },
    init = function()
      vim.api.nvim_create_autocmd("FileType", {
        pattern = { "help", "alpha", "dashboard", "neo-tree", "Trouble", "lazy", "mason", "notify", "toggleterm" },
        callback = function() vim.b.miniindentscope_disable = true end,
      })
    end,
  },

  -- ── yanky.nvim: yank history with telescope picker ─────────────────────
  {
    "gbprod/yanky.nvim",
    event = { "BufReadPost", "BufNewFile" },
    opts  = { highlight = { timer = 150 } },
    keys = {
      { "y",  "<Plug>(YankyYank)",                       mode = { "n", "x" }, desc = "Yank text" },
      { "p",  "<Plug>(YankyPutAfter)",                   mode = { "n", "x" }, desc = "Put after" },
      { "P",  "<Plug>(YankyPutBefore)",                  mode = { "n", "x" }, desc = "Put before" },
      { "<leader>fy", "<cmd>Telescope yank_history<cr>", desc = "Yank history" },
    },
    config = function(_, opts)
      require("yanky").setup(opts)
      pcall(function() require("telescope").load_extension("yank_history") end)
    end,
  },

  -- ── undotree: visualize the undo history ───────────────────────────────
  {
    "mbbill/undotree",
    cmd  = "UndotreeToggle",
    keys = { { "<leader>u", "<cmd>UndotreeToggle<cr>", desc = "Undotree" } },
  },
}
