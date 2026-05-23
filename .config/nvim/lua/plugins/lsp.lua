-- =============================================================================
-- LSP -- Language servers, completions, snippets
-- =============================================================================

-- Single source of truth for LSP servers: `ensure_installed` for mason-lspconfig
-- and (minus lua_ls, which has bespoke settings below) the list the nvim-lspconfig
-- `config` block iterates over.
local servers = {
  -- Systems / security
  "lua_ls", "pyright", "ruff", "clangd", "rust_analyzer",
  "gopls", "zls",
  -- Web
  "ts_ls", "html", "cssls", "jsonls", "tailwindcss",
  -- Config / ops
  "bashls", "yamlls", "dockerls", "terraformls",
}

return {
  -- ── Mason: portable LSP / DAP / linter / formatter installer ────────────
  {
    "williamboman/mason.nvim",
    cmd = "Mason",
    opts = {
      ui = { border = "rounded", height = 0.7 },
    },
  },

  {
    "williamboman/mason-lspconfig.nvim",
    event = { "BufReadPre", "BufNewFile" },
    dependencies = {
      "williamboman/mason.nvim",
      "neovim/nvim-lspconfig",
    },
    opts = { ensure_installed = servers },
  },

  -- ── Lazydev (Lua LS for Neovim config / plugin dev) ─────────────────────
  {
    "folke/lazydev.nvim",
    ft   = "lua",
    opts = {
      library = {
        { path = "${3rd}/luv/library", words = { "vim%.uv" } },
      },
    },
  },

  -- ── nvim-lspconfig ──────────────────────────────────────────────────────
  -- Uses the Neovim 0.11+ API: nvim-lspconfig ships per-server configs on the
  -- runtimepath as `lsp/<name>.lua`; we layer overrides via `vim.lsp.config`
  -- and activate with `vim.lsp.enable`. The old `lspconfig[server].setup({})`
  -- framework is deprecated (`:h lspconfig-nvim-0.11`).
  {
    "neovim/nvim-lspconfig",
    event = { "BufReadPre", "BufNewFile" },
    dependencies = {
      "hrsh7th/cmp-nvim-lsp",
    },
    config = function()
      local capabilities = require("cmp_nvim_lsp").default_capabilities()

      -- Per-buffer keymaps, installed on every LspAttach (replaces the old
      -- `on_attach` parameter that each setup() call used to carry).
      vim.api.nvim_create_autocmd("LspAttach", {
        group = vim.api.nvim_create_augroup("dotfiles-lsp-attach", { clear = true }),
        callback = function(ev)
          local map = function(mode, keys, func, desc)
            vim.keymap.set(mode, keys, func, { buffer = ev.buf, desc = "LSP: " .. desc })
          end
          map("n", "gd",         vim.lsp.buf.definition,      "Go to definition")
          map("n", "gD",         vim.lsp.buf.declaration,      "Go to declaration")
          map("n", "gr",         vim.lsp.buf.references,       "References")
          map("n", "gi",         vim.lsp.buf.implementation,   "Implementation")
          -- K → hover is built-in on LSP-attached buffers since Neovim 0.10.
          -- Signature help in insert mode (normal <C-k> is window-nav-up)
          map("i", "<C-k>",      function() vim.lsp.buf.signature_help({ border = "rounded" }) end, "Signature help")
          map("n", "<leader>rn", vim.lsp.buf.rename,           "Rename")
          map("n", "<leader>ca", vim.lsp.buf.code_action,      "Code action")
          map("n", "<leader>D",  vim.lsp.buf.type_definition,  "Type definition")
          map("n", "<leader>fs", vim.lsp.buf.document_symbol,  "Document symbols")
        end,
      })

      -- Shared cmp capabilities on every server via wildcard.
      vim.lsp.config("*", { capabilities = capabilities })

      -- Lua-specific settings (merged on top of the wildcard config).
      vim.lsp.config("lua_ls", {
        settings = {
          Lua = {
            workspace   = { checkThirdParty = false },
            telemetry   = { enable = false },
            completion  = { callSnippet = "Replace" },
            diagnostics = { globals = { "vim" } },
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
        float = {
          border = "rounded",
          source = true,
        },
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
      "saadparwaiz1/cmp_luasnip",
      {
        "L3MON4D3/LuaSnip",
        build = "make install_jsregexp",
        dependencies = { "rafamadriz/friendly-snippets" },
        config = function()
          require("luasnip.loaders.from_vscode").lazy_load()
        end,
      },
      "onsails/lspkind.nvim",
    },
    config = function()
      local cmp     = require("cmp")
      local luasnip = require("luasnip")
      local lspkind = require("lspkind")

      cmp.setup({
        snippet = {
          expand = function(args) luasnip.lsp_expand(args.body) end,
        },
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
          { name = "luasnip"  },
          { name = "path"     },
        }, {
          { name = "buffer"   },
        }),
        formatting = {
          format = lspkind.cmp_format({
            mode      = "symbol_text",
            maxwidth  = 50,
            ellipsis_char = "...",
          }),
        },
      })

      -- Cmdline completions
      cmp.setup.cmdline(":", {
        mapping = cmp.mapping.preset.cmdline(),
        sources = cmp.config.sources(
          { { name = "path" } },
          { { name = "cmdline" } }
        ),
      })
      cmp.setup.cmdline({ "/", "?" }, {
        mapping = cmp.mapping.preset.cmdline(),
        sources = { { name = "buffer" } },
      })
    end,
  },

  -- ── Conform: formatting ─────────────────────────────────────────────────
  {
    "stevearc/conform.nvim",
    event = "BufWritePre",
    cmd   = "ConformInfo",
    keys  = {
      { "<leader>cf", function() require("conform").format({ async = true }) end, desc = "Format buffer" },
    },
    opts = {
      formatters_by_ft = {
        lua        = { "stylua" },
        python     = { "ruff_format" },
        javascript = { "prettierd", "prettier", stop_after_first = true },
        typescript = { "prettierd", "prettier", stop_after_first = true },
        json       = { "prettierd", "prettier", stop_after_first = true },
        yaml       = { "prettierd", "prettier", stop_after_first = true },
        html       = { "prettierd", "prettier", stop_after_first = true },
        css        = { "prettierd", "prettier", stop_after_first = true },
        sh         = { "shfmt" },
        bash       = { "shfmt" },
        go         = { "gofmt" },
        rust       = { "rustfmt" },
        c          = { "clang-format" },
        cpp        = { "clang-format" },
      },
      format_on_save = {
        timeout_ms = 500,
        lsp_format = "fallback",
      },
    },
  },
}
