# texl-cli

The `texl` command: renders a [Texl](../README.md) template file to standard
output or to a file.

```sh
cargo install --path texl-cli
texl report.texl -o report.md
```

Imports inside the template resolve against the template's own folder.

See the [command-line manual](https://onlabsorg.github.io/texl/book/hosts/cli.html).

## License

[MIT](../LICENSE)
