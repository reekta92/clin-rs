# Installation

Choose a package for your platform from [Releases](https://github.com/reekta92/clin-rs/releases), or install with Cargo, AUR, or Nix.

**Before upgrading an encrypted vault:** read [Upgrading](UPGRADING.md). This release cannot decrypt legacy notes or recovery records.

## Terminal and fonts

Recommended for the best experience:

- **A Nerd Font** — recommended for `icon_mode = "nerd"`. `JetBrains Mono Nerd Font` works well; see [Nerd Fonts](https://www.nerdfonts.com/font-downloads). Use `"unicode"` for single-cell text symbols with a regular monospace font, `"emoji"` for emoji icons, or `"none"` for text labels without decorative icons. These modes also replace Powerline separators.
- **A modern terminal** — such as Kitty, Ghostty, Alacritty, or foot. Image protocol support enables native image previews. If a terminal shortcut conflicts with clin, remap it in clin or your terminal.

---

## Optional tools

These tools are **optional** — clin works without them:

| Tool | Purpose | Package |
|---|---|---|
| `tesseract` | OCR paste (clipboard image → text) | `tesseract-ocr` |
| `wl-clipboard` | Clipboard access (Wayland) | `wl-clipboard` |
| `xclip` or `xsel` | Clipboard access (X11) | `xclip` |
| `markitdown` or `pandoc` | Convert PDF/DOCX/HTML and other imported files to Markdown | `pip install markitdown` / `pandoc` |
| `trafilatura` | Extract article text and metadata for HTML URL imports; falls back to the general converters | `pip install trafilatura` |

---

## Rust requirements

Building from source or installing with Cargo requires Rust 1.90.0 or newer. Prebuilt packages do not require Rust.

## From source

```bash
git clone https://github.com/reekta92/clin-rs.git
cd clin-rs
cargo build --release --locked
```

## With Cargo

```bash
cargo install clin-rs --locked
```

## Debian/Ubuntu (.deb)

Download the latest `.deb` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo dpkg -i clin-rs_*_amd64.deb
```

## Debian/Ubuntu (aarch64)

Download the latest `.deb` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo dpkg -i clin-rs_*_arm64.deb
```

## Fedora/RHEL (.rpm)

Download the latest `.rpm` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo rpm -i clin-rs-*.x86_64.rpm
```

## Fedora/RHEL (aarch64)

Download the latest `.rpm` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
sudo rpm -i clin-rs-*.aarch64.rpm
```

## Arch Linux (PKGBUILD)

A `PKGBUILD` is included in the root of the repository.
```bash
git clone https://github.com/reekta92/clin-rs.git
cd clin-rs
makepkg -si
```

## Arch Linux (AUR)

```bash
yay -S clin-rs-bin
```

## Nix (Flakes)

*Note: Requires `nix-command` and `flakes` experimental features enabled.*

Run directly:
```bash
nix run github:reekta92/clin-rs --extra-experimental-features "nix-command flakes"
```
Install to profile:
```bash
nix profile install github:reekta92/clin-rs
```
Add to your flake `inputs`:
```nix
{
  inputs.clin.url = "github:reekta92/clin-rs";
}
```
Then include `inputs.clin.packages.${system}.default` in your `environment.systemPackages` or `home.packages`.

## AppImage

Download the latest `.AppImage` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
chmod +x clin-*-x86_64.AppImage
./clin-*-x86_64.AppImage
```

## Linux archive (x86_64)

Download the latest `.tar.gz` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
tar -xzf clin-rs-x86_64-unknown-linux-gnu.tar.gz
chmod +x clin
mkdir -p ~/.local/bin
mv clin ~/.local/bin/
```

## Linux archive (aarch64)

Download the latest `.tar.gz` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```bash
tar -xzf clin-rs-aarch64-unknown-linux-gnu.tar.gz
chmod +x clin
mkdir -p ~/.local/bin
mv clin ~/.local/bin/
```

## Windows

Download the latest `.zip` from the [Releases](https://github.com/reekta92/clin-rs/releases) page.
```powershell
Expand-Archive -Path clin-rs-x86_64-pc-windows-msvc.zip -DestinationPath clin
cd clin
.\clin.exe
```

Windows builds are provided but TUI glyph rendering depends on your terminal. [Windows Terminal](https://apps.microsoft.com/detail/9n0dx20hk701) is recommended for the best experience.

## macOS (Apple Silicon)

Download the latest `.dmg` (or `.tar.gz`) from the [Releases](https://github.com/reekta92/clin-rs/releases) page.

```bash
# Using .dmg

hdiutil attach clin-rs-aarch64-apple-darwin.dmg
cp -r /Volumes/clin/clin.app /Applications/
hdiutil detach /Volumes/clin

# Or using .tar.gz

tar -xzf clin-rs-aarch64-apple-darwin.tar.gz
chmod +x clin
mkdir -p ~/.local/bin
mv clin ~/.local/bin/
```

Apple Silicon only (no Intel build ships).
The included `clin.app` opens clin in Terminal and uses the application icon in Finder.

> **Rust not installed?** Run `curl https://sh.rustup.rs -sSf | sh` to install Rust.

Return to the [quick start](../README.md#quick-start).
