# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.0_beta.4
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.0-beta.4/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("66af8a92aa13df59c108a9b7bff19360df30f029c1ef561ca541560c0a88216b")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
