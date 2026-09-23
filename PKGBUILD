# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.13.0
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.13.0/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("25fab8013b877772677fad07f071ed8d870f5dd755a0fd1b9b53a16cc50f05ef")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
