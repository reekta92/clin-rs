# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_rc.4
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-rc.4/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("d612d0b2da0304bd5727e20e6efe1403a6421a6c60d1ed2a7638d64a5b053eed")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
