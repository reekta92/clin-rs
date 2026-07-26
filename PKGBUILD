# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_rc.6
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-rc.6/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("0ddeb2d67eb78b4a905a5eae91b5d273cc695f1bc3df23c0bddcd02756df2e86")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
