# Publishing `awesome-ani-cli-git` to the AUR

Run these on your Arch machine (they need `pacman`/`makepkg`, which this repo's CI box does not have).

## 1. Create an AUR account + SSH key (one time)

1. Register at https://aur.archlinux.org/register (any username, e.g. `mgh12453`).
2. Generate an SSH key if you don't have one:
   ```sh
   ssh-keygen -t ed25519 -C "aur" -f ~/.ssh/aur
   ```
3. Copy the **public** key and paste it into AUR → *My Account* → *SSH Public Key*:
   ```sh
   cat ~/.ssh/aur.pub
   ```
4. Tell SSH to use it for the AUR host — add to `~/.ssh/config`:
   ```
   Host aur.archlinux.org
       IdentityFile ~/.ssh/aur
       User aur
   ```

## 2. Create and push the package

```sh
# clone the (empty) AUR repo for the package name
git clone ssh://aur@aur.archlinux.org/awesome-ani-cli-git.git
cd awesome-ani-cli-git

# copy the PKGBUILD from this repo
cp /path/to/awesome-ani-cli/packaging/aur/PKGBUILD .

# build + install locally to verify it works
makepkg -si

# generate the metadata AUR requires
makepkg --printsrcinfo > .SRCINFO

git add PKGBUILD .SRCINFO
git commit -m "Initial import of awesome-ani-cli-git"
git push
```

After the push it is installable with:

```sh
yay -S awesome-ani-cli-git
```

## Notes

- The package installs the binary as `ani-cli` and declares `provides=ani-cli` /
  `conflicts=ani-cli`, so it cannot be co-installed with the upstream `ani-cli`
  package (it is a superset).
- It is a `-git` package: `pkgver()` is derived from the git history, so bumping is
  automatic — re-run `makepkg --printsrcinfo > .SRCINFO` and push when you cut changes.
