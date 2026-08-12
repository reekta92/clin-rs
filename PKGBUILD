# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.1
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.1/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("c486872be3569d3e26a4b6838bccf5f292271467ba8ab1df8e12580b0a4f9602")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
