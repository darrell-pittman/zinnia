return {
  {
    name = 'Cargo',
    pattern = '*.rs',
    validate = {
      static = {
        exe_exists = 'cargo',
        tests = {
          function()
            local found = vim.fs.find('Cargo.toml', { path = vim.uv.cwd() })
            return found and #found == 1
          end,
        },
      },
    },
    run_cmd = {
      'cargo',
      'run'
    }
  }
}
