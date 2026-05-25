-- =============================================================================
-- LSP -- Language servers, completion, snippets, formatters, linters
-- =============================================================================

-- Single source of truth for LSP servers: `ensure_installed` for
-- mason-lspconfig and the wildcard config we layer over.
local servers = {
  -- Lua
  "lua_ls",
  -- Systems
  "clangd", "rust_analyzer", "gopls", "zls", "cmake",
  -- Python
  "pyright", "ruff",
  -- JS / TS / web frameworks
  "ts_ls", "vue_ls", "svelte", "astro", "html", "cssls", "tailwindcss",
  "emmet_ls", "jsonls", "prismals", "graphql",
  -- Markup / docs
  "marksman", "texlab",
  -- Config / IaC
  "taplo", "yamlls", "dockerls", "terraformls", "helm_ls", "ansiblels",
  "lemminx", "sqlls",
  -- Shell
  "bashls",
  -- Other languages
  "elixirls", "hls", "ocamllsp", "nil_ls", "intelephense", "solargraph",
  "jdtls", "kotlin_language_server",
  -- Editor / config
  "vimls",
}

-- Extra Mason packages that aren't LSPs (formatters, linters, DAP adapters).
local mason_tools = {
  -- Formatters
  "stylua", "prettierd", "prettier", "shfmt", "black", "isort",
  "yamlfmt", "alejandra", "buf", "gofumpt", "goimports", "clang-format",
  -- Linters
  "shellcheck", "hadolint", "yamllint", "markdownlint", "ansible-lint",
  "eslint_d", "pylint", "mypy", "checkmake", "luacheck", "gitlint",
  -- DAP adapters
  "debugpy", "codelldb", "delve", "js-debug-adapter",
}

return {
  -- ── Mason ────────────────────────────────────────────────────────────────
  {
    "williamboman/mason.nvim",
    cmd = "Mason",
    opts = { ui = { border = "rounded", height = 0.7 } },
  },

  {
    "williamboman/mason-lspconfig.nvim",
    event = { "BufReadPre", "BufNewFile" },
    dependencies = { "williamboman/mason.nvim", "neovim/nvim-lspconfig" },
    opts = { ensure_installed = servers },
  },

  -- Auto-install non-LSP tools (formatters, linters, DAP adapters).
  {
    "WhoIsSethDaniel/mason-tool-installer.nvim",
    event = { "BufReadPre", "BufNewFile" },
    dependencies = { "williamboman/mason.nvim" },
    opts = {
      ensure_installed = mason_tools,
      run_on_start = true,
      auto_update = false,
    },
  },

  -- ── Lazydev (lua_ls for Neovim config / plugin dev) ─────────────────────
  {
    "folke/lazydev.nvim",
    ft = "lua",
    opts = {
      library = {
        { path = "${3rd}/luv/library", words = { "vim%.uv" } },
      },
    },
  },

  -- ── nvim-lspconfig (Neovim 0.11+ API) ───────────────────────────────────
  {
    "neovim/nvim-lspconfig",
    event = { "BufReadPre", "BufNewFile" },
    dependencies = { "hrsh7th/cmp-nvim-lsp" },
    config = function()
      local capabilities = require("cmp_nvim_lsp").default_capabilities()

      -- Per-buffer keymaps on every LspAttach.
      vim.api.nvim_create_autocmd("LspAttach", {
        group = vim.api.nvim_create_augroup("dotfiles-lsp-attach", { clear = true }),
        callback = function(ev)
          local map = function(mode, keys, func, desc)
            vim.keymap.set(mode, keys, func, { buffer = ev.buf, desc = "LSP: " .. desc })
          end
          map("n", "gd",         vim.lsp.buf.definition,     "Go to definition")
          map("n", "gD",         vim.lsp.buf.declaration,    "Go to declaration")
          map("n", "gr",         vim.lsp.buf.references,     "References")
          map("n", "gi",         vim.lsp.buf.implementation, "Implementation")
          map("i", "<C-k>",      function() vim.lsp.buf.signature_help({ border = "rounded" }) end, "Signature help")
          map("n", "<leader>rn", vim.lsp.buf.rename,          "Rename")
          map("n", "<leader>ca", vim.lsp.buf.code_action,     "Code action")
          map("n", "<leader>D",  vim.lsp.buf.type_definition, "Type definition")
          map("n", "<leader>fs", vim.lsp.buf.document_symbol, "Document symbols")

          -- Inlay hints (Neovim 0.10+)
          if vim.lsp.inlay_hint then
            pcall(vim.lsp.inlay_hint.enable, true, { bufnr = ev.buf })
            map("n", "<leader>ih", function()
              vim.lsp.inlay_hint.enable(not vim.lsp.inlay_hint.is_enabled({ bufnr = ev.buf }), { bufnr = ev.buf })
            end, "Toggle inlay hints")
          end

          -- Document highlight when the LSP supports it.
          local client = vim.lsp.get_client_by_id(ev.data.client_id)
          if client and client.server_capabilities.documentHighlightProvider then
            local hl_group = vim.api.nvim_create_augroup("dotfiles-lsp-highlight-" .. ev.buf, { clear = true })
            vim.api.nvim_create_autocmd({ "CursorHold", "CursorHoldI" }, {
              buffer = ev.buf, group = hl_group, callback = vim.lsp.buf.document_highlight,
            })
            vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI" }, {
              buffer = ev.buf, group = hl_group, callback = vim.lsp.buf.clear_references,
            })
          end
        end,
      })

      -- Shared cmp capabilities on every server via wildcard.
      vim.lsp.config("*", { capabilities = capabilities })

      -- Lua-specific overrides (merged on top of the wildcard config).
      vim.lsp.config("lua_ls", {
        settings = {
          Lua = {
            workspace   = { checkThirdParty = false },
            telemetry   = { enable = false },
            completion  = { callSnippet = "Replace" },
            diagnostics = { globals = { "vim" } },
            hint        = { enable = true },
          },
        },
      })

      -- Rust-analyzer settings (rustaceanvim handles activation).
      vim.lsp.config("rust_analyzer", {
        settings = {
          ["rust-analyzer"] = {
            cargo      = { allFeatures = true, loadOutDirsFromCheck = true },
            checkOnSave = { command = "clippy" },
            inlayHints = { closingBraceHints = { enable = true }, parameterHints = { enable = true } },
          },
        },
      })

      vim.lsp.enable(servers)

      vim.diagnostic.config({
        virtual_text     = { prefix = "" },
        signs            = true,
        underline        = true,
        update_in_insert = false,
        severity_sort    = true,
        float            = { border = "rounded", source = true },
      })
    end,
  },

  -- ── nvim-cmp: autocompletion engine ─────────────────────────────────────
  {
    "hrsh7th/nvim-cmp",
    event = "InsertEnter",
    dependencies = {
      "hrsh7th/cmp-nvim-lsp",
      "hrsh7th/cmp-buffer",
      "hrsh7th/cmp-path",
      "hrsh7th/cmp-cmdline",
      "hrsh7th/cmp-nvim-lsp-signature-help",
      "hrsh7th/cmp-nvim-lua",
      "hrsh7th/cmp-emoji",
      "saadparwaiz1/cmp_luasnip",
      {
        "L3MON4D3/LuaSnip",
        build = "make install_jsregexp",
        dependencies = { "rafamadriz/friendly-snippets" },
        config = function() require("luasnip.loaders.from_vscode").lazy_load() end,
      },
      "onsails/lspkind.nvim",
    },
    config = function()
      local cmp     = require("cmp")
      local luasnip = require("luasnip")
      local lspkind = require("lspkind")

      cmp.setup({
        snippet = { expand = function(args) luasnip.lsp_expand(args.body) end },
        window = {
          completion    = cmp.config.window.bordered(),
          documentation = cmp.config.window.bordered(),
        },
        mapping = cmp.mapping.preset.insert({
          ["<C-n>"]     = cmp.mapping.select_next_item(),
          ["<C-p>"]     = cmp.mapping.select_prev_item(),
          ["<C-b>"]     = cmp.mapping.scroll_docs(-4),
          ["<C-f>"]     = cmp.mapping.scroll_docs(4),
          ["<C-Space>"] = cmp.mapping.complete(),
          ["<C-e>"]     = cmp.mapping.abort(),
          ["<CR>"]      = cmp.mapping.confirm({ select = false }),
          ["<Tab>"]     = cmp.mapping(function(fallback)
            if cmp.visible() then cmp.select_next_item()
            elseif luasnip.expand_or_jumpable() then luasnip.expand_or_jump()
            else fallback() end
          end, { "i", "s" }),
          ["<S-Tab>"]   = cmp.mapping(function(fallback)
            if cmp.visible() then cmp.select_prev_item()
            elseif luasnip.jumpable(-1) then luasnip.jump(-1)
            else fallback() end
          end, { "i", "s" }),
        }),
        sources = cmp.config.sources({
          { name = "nvim_lsp" },
          { name = "nvim_lsp_signature_help" },
          { name = "luasnip" },
          { name = "lazydev", group_index = 0 },
          { name = "nvim_lua" },
          { name = "path" },
        }, {
          { name = "buffer" },
          { name = "emoji" },
        }),
        formatting = {
          format = lspkind.cmp_format({
            mode = "symbol_text", maxwidth = 50, ellipsis_char = "...",
          }),
        },
      })

      cmp.setup.cmdline(":", {
        mapping = cmp.mapping.preset.cmdline(),
        sources = cmp.config.sources({ { name = "path" } }, { { name = "cmdline" } }),
      })
      cmp.setup.cmdline({ "/", "?" }, {
        mapping = cmp.mapping.preset.cmdline(),
        sources = { { name = "buffer" } },
      })
    end,
  },

  -- ── conform: formatters ─────────────────────────────────────────────────
  {
    "stevearc/conform.nvim",
    event = "BufWritePre",
    cmd = "ConformInfo",
    keys = {
      { "<leader>cf", function() require("conform").format({ async = true }) end, desc = "Format buffer" },
    },
    opts = {
      formatters_by_ft = {
        lua             = { "stylua" },
        python          = { "ruff_format", "ruff_organize_imports" },
        javascript      = { "prettierd", "prettier", stop_after_first = true },
        typescript      = { "prettierd", "prettier", stop_after_first = true },
        javascriptreact = { "prettierd", "prettier", stop_after_first = true },
        typescriptreact = { "prettierd", "prettier", stop_after_first = true },
        vue             = { "prettierd", "prettier", stop_after_first = true },
        svelte          = { "prettierd", "prettier", stop_after_first = true },
        astro           = { "prettierd", "prettier", stop_after_first = true },
        json            = { "prettierd", "prettier", stop_after_first = true },
        jsonc           = { "prettierd", "prettier", stop_after_first = true },
        yaml            = { "yamlfmt" },
        html            = { "prettierd", "prettier", stop_after_first = true },
        css             = { "prettierd", "prettier", stop_after_first = true },
        scss            = { "prettierd", "prettier", stop_after_first = true },
        markdown        = { "prettierd", "prettier", stop_after_first = true },
        graphql         = { "prettierd", "prettier", stop_after_first = true },
        sh              = { "shfmt" },
        bash            = { "shfmt" },
        zsh             = { "shfmt" },
        go              = { "goimports", "gofumpt" },
        rust            = { "rustfmt" },
        c               = { "clang-format" },
        cpp             = { "clang-format" },
        toml            = { "taplo" },
        nix             = { "alejandra" },
        proto           = { "buf" },
        terraform       = { "terraform_fmt" },
        ["*"]           = { "trim_whitespace", "trim_newlines" },
      },
      format_on_save = { timeout_ms = 500, lsp_format = "fallback" },
    },
  },

  -- ── nvim-lint: linters ─────────────────────────────────────────────────
  {
    "mfussenegger/nvim-lint",
    event = { "BufReadPre", "BufNewFile" },
    config = function()
      require("lint").linters_by_ft = {
        python          = { "pylint", "mypy" },
        javascript      = { "eslint_d" },
        typescript      = { "eslint_d" },
        javascriptreact = { "eslint_d" },
        typescriptreact = { "eslint_d" },
        sh              = { "shellcheck" },
        bash            = { "shellcheck" },
        zsh             = { "shellcheck" },
        dockerfile      = { "hadolint" },
        yaml            = { "yamllint" },
        markdown        = { "markdownlint" },
        ansible         = { "ansible-lint" },
        make            = { "checkmake" },
        gitcommit       = { "gitlint" },
        lua             = { "luacheck" },
      }
      local lint_group = vim.api.nvim_create_augroup("dotfiles-lint", { clear = true })
      vim.api.nvim_create_autocmd({ "BufWritePost", "BufReadPost", "InsertLeave" }, {
        group = lint_group,
        -- Silently skip linters whose binaries aren't on $PATH yet
        -- (mason-tool-installer pulls them in the background).
        callback = function()
          local lint = require("lint")
          local linters = lint.linters_by_ft[vim.bo.filetype] or {}
          local available = {}
          for _, name in ipairs(linters) do
            local cfg = lint.linters[name]
            local cmd = type(cfg) == "table" and (cfg.cmd or name) or name
            if vim.fn.executable(cmd) == 1 then table.insert(available, name) end
          end
          if #available > 0 then pcall(lint.try_lint, available) end
        end,
      })
    end,
  },

  -- ── fidget: LSP progress in the corner ──────────────────────────────────
  {
    "j-hui/fidget.nvim",
    event = "LspAttach",
    opts = {
      progress     = { display = { done_icon = "" } },
      notification = { window = { winblend = 0 } },
    },
  },

  -- ── lsp_signature: live signature help in insert mode ──────────────────
  {
    "ray-x/lsp_signature.nvim",
    event = "LspAttach",
    opts = {
      bind            = true,
      handler_opts    = { border = "rounded" },
      hint_enable     = false,
      floating_window = false,
    },
  },

  -- ── nvim-navic: breadcrumbs (powers barbecue winbar) ───────────────────
  {
    "SmiteshP/nvim-navic",
    dependencies = { "neovim/nvim-lspconfig" },
    opts = { highlight = true, separator = "  ", depth_limit = 5 },
    init = function()
      vim.api.nvim_create_autocmd("LspAttach", {
        callback = function(ev)
          local client = vim.lsp.get_client_by_id(ev.data.client_id)
          if client and client.server_capabilities.documentSymbolProvider then
            require("nvim-navic").attach(client, ev.buf)
          end
        end,
      })
    end,
  },

  -- ── aerial.nvim: code outline ──────────────────────────────────────────
  {
    "stevearc/aerial.nvim",
    cmd = { "AerialToggle", "AerialOpen", "AerialClose" },
    keys = {
      { "<leader>fo", "<cmd>AerialToggle<cr>", desc = "Toggle code outline" },
    },
    opts = {
      backends     = { "lsp", "treesitter", "markdown", "man" },
      layout       = { default_direction = "right", min_width = 28, max_width = 40 },
      attach_mode  = "global",
      filter_kind  = false,
    },
  },

  -- ── garbage-day: stop idle LSPs to save RAM ────────────────────────────
  {
    "zeioth/garbage-day.nvim",
    dependencies = "neovim/nvim-lspconfig",
    event = "VeryLazy",
    opts = { aggressive_mode = false, grace_period = 60 * 15 },
  },

  -- ── actions-preview: code-action diff popup ────────────────────────────
  {
    "aznhe21/actions-preview.nvim",
    keys = {
      { "<leader>cA", function() require("actions-preview").code_actions() end,
        mode = { "n", "v" }, desc = "Code action (preview)" },
    },
  },

  -- ── inc-rename: live LSP rename ────────────────────────────────────────
  {
    "smjonas/inc-rename.nvim",
    cmd = "IncRename",
    keys = {
      { "<leader>rN", function() return ":IncRename " .. vim.fn.expand("<cword>") end,
        expr = true, desc = "Rename (incremental)" },
    },
    opts = {},
  },
}
