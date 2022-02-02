# Contribution Guidelines

## Pull Requests

### Rules

1. Appease the linter
2. Bump the version
3. Adjust the Readme according to your changes (if applicable)
4. No extra dependencies unless absolutely necessary

### Guidelines

5. Try using built-in output functions (err, inf, prompt, `menu_line_*` and die) instead of echo and printf
6. Don't echo-pipe into another command if avoidable
7. Indent with tabs
8. Try using shell builtins over external commands
9. Use [shellcheck](https://github.com/koalaman/shellcheck) before pushing (recommended)

## Testing

Our parsing was broken in the past and it will break in the future

To spot breakage early, test with the following anime:

- The safe bet: `One Piece`
- Episode 5.5: `arifureta shokugyou de sekai saikyou`
- Unicode: `Saenai Heroine no Sodatekata ♭`
- Unreleased: `boku-no-hero-academia-the-movie-3`
- Old anime: `Paprika`

Test automation ideas welcome

## How else can I help?

- Join the [discord](https://discord.gg/aqu7GpqVmR)
- Vote on polls there
- Star the repo
- Follow the maintainers
