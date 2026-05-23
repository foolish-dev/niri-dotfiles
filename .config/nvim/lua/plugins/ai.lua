-- =============================================================================
-- AI -- opencode.nvim integration
-- =============================================================================
return {
  {
    "NickvanDyke/opencode.nvim",
    version = "*",
    init = function()
      ---@type opencode.Opts
      vim.g.opencode_opts = {}
      vim.o.autoread = true -- required for opts.events.reload
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
}
