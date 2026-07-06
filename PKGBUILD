# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.6
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.6/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("6f0970c9487935e76fe6e071f4b975061bc4823abee63d047500f3f1be103a7e")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
