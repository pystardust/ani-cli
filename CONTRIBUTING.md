# How to contribute

## Issues: Use an Issue template if applicable

## Support: Join our [discord](https://discord.gg/aqu7GpqVmR)

## Pull Requests

- Appease the linter
- Don't echo-pipe into another command
- Adjust the Readme according to your changes (if applicable)
- Indent with tabs and clear trailing white space before commits.
- Bump the version
- Try using built-in output functions (err, inf, prompt, `menu_line_*` and die) instead of echo and printf
- Try using shell builtins over external commands
- Avoid extra dependencies

## Testing

Our parsing was broken in the past and it will break in the future

To spot breakage early, test with the following anime:

- The safe bet: `One Piece`
- Episode 5.5: `arifureta shokugyou de sekai saikyou`
- Unicode: `Saenai Heroine no Sodatekata ♭`
- Unreleased: `boku-no-hero-academia-the-movie-3`

Test automation ideas welcome

## How else can I help?

- Join the [discord](https://discord.gg/aqu7GpqVmR)
- Vote on polls there
- Star the repo
- Follow the maintainers
