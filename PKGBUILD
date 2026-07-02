# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.3
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.3/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("41075c4bcce84e2a33a71db7c6777c21d7b121827287479129cd51a0ecc98c04")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
