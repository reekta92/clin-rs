# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.13.0_testing.2
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.13.0-testing.2/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("03b9bf3cab6506d14e6d4d0b1f1b2c84d7dd6e575bd67f879ec97b985ea875ad")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
