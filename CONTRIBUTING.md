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
10. Test using the dash shell, since it's strictly posix compliant

## Advice for maintainers

1. Always use [semantic commits](https://gist.github.com/joshbuchea/6f47e86d2510bce28f8e7f42ae84c716) when merging to master
2. Remove features. Who will do it if you don't?
3. Use labels
4. Link PRs to the issues they solve
5. Assign issues if you know who will/should solve them
6. Close stale, unreproducible and low quality issues
7. Don't use projects or milestones, they don't add any value to the workflow
8. Don't advertise to the wrong demographic
9. Don't advertise
10. Help new contributors find things to work on

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
