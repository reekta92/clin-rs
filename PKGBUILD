# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.14.0_beta.0
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.14.0-beta.0/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("5032c4907e6997a2c98a215bf8e9047a4082395ca4a2e990de2ffbb36f06048e")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
    # Older release archives contain only the executable.
    if [[ -f assets/clin.desktop ]]; then
        install -Dm644 "assets/clin.desktop" "$pkgdir/usr/share/applications/clin.desktop"
        install -Dm644 "assets/icon.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/clin.svg"
        install -Dm644 "assets/clin.png" "$pkgdir/usr/share/icons/hicolor/256x256/apps/clin.png"
    fi
}
