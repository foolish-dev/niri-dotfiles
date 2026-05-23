-- =============================================================================
-- UI -- statusline, bufferline, dashboard, noice, dressing
-- =============================================================================
return {
  -- ── Lualine (statusline) ────────────────────────────────────────────────
  {
    "nvim-lualine/lualine.nvim",
    lazy = false,
    dependencies = { "nvim-tree/nvim-web-devicons" },
    opts = {
      options = {
        theme              = "tokyonight",
        globalstatus       = true,
        component_separators = { left = "", right = "" },
        section_separators   = { left = "", right = "" },
        disabled_filetypes   = { statusline = { "dashboard", "alpha" } },
      },
      sections = {
        lualine_a = { "mode" },
        lualine_b = { "branch", "diff", "diagnostics" },
        lualine_c = { { "filename", path = 1, symbols = { modified = " ", readonly = " " } } },
        lualine_x = { "encoding", "fileformat", "filetype" },
        lualine_y = { "progress" },
        lualine_z = { "location" },
      },
      extensions = { "neo-tree", "lazy", "trouble" },
    },
  },

  -- ── Bufferline ──────────────────────────────────────────────────────────
  {
    "akinsho/bufferline.nvim",
    lazy = false,
    dependencies = { "nvim-tree/nvim-web-devicons" },
    opts = {
      options = {
        diagnostics         = "nvim_lsp",
        always_show_bufferline = true,
        offsets = {
          { filetype = "neo-tree", text = "File Explorer", highlight = "Directory", separator = true },
        },
        separator_style = "thin",
      },
    },
    keys = {
      { "<leader>bp", "<cmd>BufferLineTogglePin<cr>",    desc = "Pin buffer" },
      { "<leader>bo", "<cmd>BufferLineCloseOthers<cr>",  desc = "Close other buffers" },
    },
  },

  -- ── Dashboard (alpha) ──────────────────────────────────────────────────
  {
    "goolord/alpha-nvim",
    lazy = false,
    dependencies = { "nvim-tree/nvim-web-devicons" },
    config = function()
      local alpha     = require("alpha")
      local dashboard = require("alpha.themes.dashboard")

      dashboard.section.header.val = {
        [[                                                    ]],
        [[    ⣴⣶⣤⡤⠦⣤⣀⣤⠆     ⣈⣭⣿⣶⣿⣦⣼⣆                    ]],
        [[     ⠉⠻⢿⣿⠿⣿⣿⣶⣦⠤⠄⡐⢶⣯⣭⣭⣭⣭⣭⣭⣭⣭⣭⣽⣿⣿⣶⣄                ]],
        [[      ⠈⠻⣿⣿⣿⣿⣿⣿⣿⣿⡁   ⢈⣿⣿⡿⠿⠛⢻⣯⣭⣭⣽⣿⣿⣿⣿⣶⣄            ]],
        [[       ⠈⠈⠙⢿⣿⣿⣿⣿⣿⣟⣦⡄⢿⠈⠛⠛   ⠁⠓⠉⠙⠛⠛⠉⠟⠉⠛⠛⣿⣿⣿⣿⣶⡄        ]],
        [[           ⠉⠛⢿⣿⣿⣿⣿⣿⣿⣷⡀       ⢀⣤⣤⣤⣀⣤⣤⣴⣿⣿⣿⣿⣿⣿⣿⡄       ]],
        [[        ⠉⠻⣿⣿⣿⣿⣿⣿⣿⠿⠛          ⣴⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿       ]],
        [[        ⠻⣿⣿⣿⡿⠿⠛              ⣿⡟⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿       ]],
        [[         ⠙⠁                  ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿       ]],
        [[                             ⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿        ]],
        [[                              ⠻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠟        ]],
        [[                               ⠈⠻⣿⣿⣿⣿⣿⣿⣿⣿          ]],
        [[                                                    ]],
        [[              ⟨  N E O V I M  //  H A C K  ⟩              ]],
        [[                                                    ]],
      }

      dashboard.section.buttons.val = {
        dashboard.button("f", "  Find file",       "<cmd>Telescope find_files<cr>"),
        dashboard.button("g", "  Live grep",       "<cmd>Telescope live_grep<cr>"),
        dashboard.button("r", "  Recent files",    "<cmd>Telescope oldfiles<cr>"),
        dashboard.button("n", "  New file",        "<cmd>ene <BAR> startinsert<cr>"),
        dashboard.button("c", "  Config",          "<cmd>e $MYVIMRC<cr>"),
        dashboard.button("l", "  Lazy",            "<cmd>Lazy<cr>"),
        dashboard.button("m", "  Mason",           "<cmd>Mason<cr>"),
        dashboard.button("q", "  Quit",            "<cmd>qa<cr>"),
      }

      dashboard.section.header.opts.hl  = "AlphaHeader"
      dashboard.section.buttons.opts.hl = "AlphaButtons"
      dashboard.section.footer.opts.hl  = "AlphaFooter"

      dashboard.section.footer.val = "// 0x000 -- ready"

      alpha.setup(dashboard.opts)
    end,
  },

  -- ── Noice (better cmdline / messages / popups) ──────────────────────────
  {
    "folke/noice.nvim",
    lazy = false,
    dependencies = { "MunifTanjim/nui.nvim", "rcarriga/nvim-notify" },
    opts = {
      lsp = {
        override = {
          -- vim.lsp.util.convert_input_to_markdown_lines and stylize_markdown
          -- were removed in Neovim 0.11 -- only override cmp's entry docs.
          ["cmp.entry.get_documentation"] = true,
        },
      },
      presets = {
        bottom_search         = true,
        command_palette       = true,
        long_message_to_split = true,
        inc_rename            = true,
        lsp_doc_border        = true,
      },
    },
  },

  -- ── Dressing (better vim.ui.select / vim.ui.input) ─────────────────────
  {
    "stevearc/dressing.nvim",
    lazy = false,
  },

  -- ── Notify ──────────────────────────────────────────────────────────────
  {
    "rcarriga/nvim-notify",
    lazy = false,
    opts = {
      timeout    = 3000,
      max_height = function() return math.floor(vim.o.lines * 0.75) end,
      max_width  = function() return math.floor(vim.o.columns * 0.75) end,
      render     = "wrapped-compact",
      stages     = "fade",
    },
  },
}
