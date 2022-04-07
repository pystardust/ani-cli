# Contribution Guidelines

## Pull Requests

- Appease the linter
- Bump the version
- Adjust the Readme according to your changes (if applicable)
- No extra dependencies unless absolutely necessary
- If you're fixing an issue, open an issue as well or link existing one

## Issues

- Use the (correct) issue template
- When requesting a feature, check it's not already in our [Rejected Feature Requests](https://github.com/pystardust/ani-cli/issues/523)
- Provide screenshot if applicable

## Testing

### Testcases

Our parsing was broken in the past and it will break in the future

To spot breakage early, test with the following anime:

- The safe bet: `One Piece`
- Episode 0: `Saenai Heroine no Sodatekata ♭`
- Unicode: `Saenai Heroine no Sodatekata ♭`
- Not uploaded: `one piece dub` episode 590
- Unreleased: `soredemo ayumu wa yosetekuru`
- Short id (for decryption): `Log Horizon` episode 1-2

### Release checklist

- any anime playing
- all testcases playing
- next, prev and replay work
- quality works
- quality works with downloads
- download works
- select episode -a and rapid resume work
- autoplay, aka range selection, works
- no shellcheck complaints

## How else can I help?

- Join the [discord](https://discord.gg/aqu7GpqVmR)
- Take part in troubleshooting and testing
- Star the repo
- Follow the maintainers
