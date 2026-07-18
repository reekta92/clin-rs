# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_beta.5
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-beta.5/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("0800848be4080b362d5d57fbf866a5422a05b0525364968856b755891337d89b")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
