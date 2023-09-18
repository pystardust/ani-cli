# Contribution Guidelines

## Pull Requests

- Appease the linter
- Bump the version
- Adjust the Readme according to your changes (if applicable)
- No extra dependencies unless absolutely necessary
- If you're fixing an issue, open an issue as well or link existing one

## Issues

- Use the issue templates
- When requesting a feature, check it hasn't been [rejected](https://github.com/pystardust/ani-cli/issues/523) previously
- Provide screenshot if applicable

## How else can I help?

- Join the [discord](https://discord.gg/aqu7GpqVmR)
- Take part in troubleshooting and testing
- Star the repo
- Follow the maintainers

## Development with nix

When you develop with nix you can use the [dev shell](https://github.com/pystardust/ani-cli#nix-shell).
To run the dev shell you can run the following command in the repository root: `nix-shell`
The dev shell includes the following packages:
- runtime dependencies of ani-cli
- shfmt
- shellcheck
Its also possible to use alternative packages for the video player or add features with this command:
```shell
nix-shell --arg <feature> true
```
These are the packages available in the dev shell:
- `withVlc`
- `withIina`
- `chromecastSupport`
- `syncSupport`
Just chain these commands together when you wanna multiple features for example:
```shell
nix-shell --arg withVlc true --arg chromecastSupport true
```
