# Pull Request Template

[Contributing guidelines](https://github.com/pystardust/ani-cli/blob/master/CONTRIBUTING.md) · [CI workflows](https://github.com/pystardust/ani-cli/tree/master/.github/workflows)

## Type of change

- [ ] Bug fix
- [ ] Feature
- [ ] Documentation update

## Description

*ramble here*

*Which flags, players or platforms does this affect?*

*Use the fixes keyword to link an issue if this directly resolves one.*

Fixes #xyz

### Verification

*Explain briefly how we can verify your results. \
Specific anime are the most valuable for bug fixes. \
Quick example invocation is most valuable for features. \
No need to restate documentation updates*

e.g. `./ani-cli -d -q 720 cyberpunk 2 -e 1 && mpv cyber*`

*(prepend the ./ since during development, we work within the repo)*

## Checklist

- [ ] any anime playing
- [ ] bumped version
---
- [ ] next, prev, replay and select work
- [ ] `-c` history and continue work
- [ ] `-d` downloads work
- [ ] `-e` (select episode) aka `-r` (range selection) works
- [ ] `-S` select index works
- [ ] `-q` quality works
- [ ] `-s` syncplay works
- [ ] `-v` vlc works
- [ ] `--dub` and regular (sub) mode both work
- [ ] `--nextep-countdown` countdown to next ep works
---
- [ ] `-h` help info is up to date
- [ ] Readme is up to date
- [ ] Man page is up to date

## Additional Testcases

*(these don't block a merge)*

- [ ] `--skip` ani-skip works
- [ ] `--no-detach` no detach works
- [ ] `--exit-after-play` auto exit after playing works

## Copy Pasta

```
./ani-cli -V
./ani-cli -S 1 -e 1 -d flcl
./ani-cli -S 1 -e 1 -q worst -v flcl
./ani-cli -c -s --dub
```
