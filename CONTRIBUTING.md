# Contribution Guidelines

## Pull Requests

- Appease the linter
- Bump the version
- Adjust the Readme according to your changes (if applicable)
- No extra dependencies unless absolutely necessary
- If you're fixing an issue, open an issue as well or link existing one

### Coding Tips

- Keep it brief. Your likelihood of being merged is inversely proportional to your change size.
- Use && and || over if-else constructs whereever appropriate
- Keep posix compliance and cross platform portability in mind

### AI Policy

- Using LLMs as a better search engine is okay
- Using LLMs to remember syntax and idioms is okay
- Using LLMs to verify posix compliance is encouraged
- Opening fully AI generated PRs is not okay
- AI should be your copilot, not you the copilot of AI
- Be cautious that LLMs tend to be overly verbose, while the ani-cli codebase prefers brevity
- Low effort PRs will be closed

Example Prompt:
```
Help me write a posix compliant shell script. No non-compliant bash idioms.
The script should work cross plattform on windows, macos and linux.
Be as brief as possible.
Use && and || over if-else constructs wherever possible.
If you use third party dependencies, tell me.
Grep, Sed and Curl are allowed, Awk and Wget are not.
(if it's a search enabled model, add a link to this contributing.md as well as our github actions file)
```


## Issues

- Use the issue templates
- When requesting a feature, check it hasn't been [rejected](https://github.com/pystardust/ani-cli/issues/523) previously
- Provide screenshot if applicable

## How else can I help?

- Join the [discord](https://discord.gg/aqu7GpqVmR)
- Take part in troubleshooting and testing
- Star the repo
- Follow the maintainers
