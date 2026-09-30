# Contribution Guidelines

## Pull Requests

- Appease the linter (run `shfmt -i 4 -ci -d -w ani-cli`)
- Appease POSIX (run `shellcheck -s sh -o all -e 2250 ani-cli`)
- Bump the version
- Adjust the Readme according to your changes (if applicable)
- No extra dependencies unless absolutely necessary
- If you're fixing an issue, open an issue as well or link existing one

### Coding Tips

- Keep it brief. Your likelihood of being merged is inversely proportional to your change size.
- Use && and || over if-else constructs whereever appropriate
- Keep posix compliance and cross platform portability in mind

### AI Policy

- AI is not allowed to write code comments
- Do not AI generate PR descriptions, it's insulting.
  Write one human sentence, or even leave it blank instead.
- Low effort PRs will be closed, especially ones violating the PR description and code comment rule.
- Add the AI model as a coauthor
- Using LLMs as a better search engine is okay
- Using LLMs to remember syntax and idioms is okay
- Using LLMs to verify posix compliance is okay
- Opening fully AI generated PRs is not okay
- Be cautious that LLMs tend to be overly verbose, while the ani-cli codebase prefers brevity

Add these two urls into the context, however you do that with your LLM of choice:
- https://github.com/pystardust/ani-cli/blob/master/.github/workflows/ani-cli.yml
- https://github.com/pystardust/ani-cli/blob/master/CONTRIBUTING.md

### Email

If you don't have a GitHub account, or you prefer to privately contribute, there is an alternative.
Send email patches or pull requests to `port19@port19.xyz`.
For privacy, mind committer name and email and tell me if there is anything I should keep in mind.

Read [the docs](https://git-scm.com/book/en/v2/Distributed-Git-Contributing-to-a-Project) for git request-pull, git format-patch, [git send-email](https://git-send-email.io/) and [git diff](https://stackoverflow.com/questions/4610744/can-i-get-a-patch-compatible-output-from-git-diff).

The guidelines that apply to pull requests also apply to email patches.

## Issues

- Use the issue templates
- When requesting a feature, check it hasn't been [rejected](https://github.com/pystardust/ani-cli/issues/523) previously
- Provide screenshot if applicable

## How else can I help?

- Join the [discord](https://discord.gg/aqu7GpqVmR)
- Take part in troubleshooting and testing
- Star the repo
- Follow the maintainers
