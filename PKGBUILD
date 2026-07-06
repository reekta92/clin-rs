# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.5
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.5/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("33a11331c133c57348cae17da52257157e3f87d40bba2a3fcf5e5d5a6dfebd36")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
