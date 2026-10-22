# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.14.0_rc.0
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.14.0-rc.0/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("4cb70196275e342354bfdfbff43dd05783c58364005ec1a9f813ba6fca7f7ac5")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
    # Older release archives contain only the executable.
    if [[ -f assets/clin.desktop ]]; then
        install -Dm644 "assets/clin.desktop" "$pkgdir/usr/share/applications/clin.desktop"
        install -Dm644 "assets/icon.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/clin.svg"
        install -Dm644 "assets/clin.png" "$pkgdir/usr/share/icons/hicolor/256x256/apps/clin.png"
    fi
}
