# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.0_rc.3
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.0-rc.3/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("f456664cad705436fade0b21de9b556104e0244fa00835bd57e31f8c4aa50f0e")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
