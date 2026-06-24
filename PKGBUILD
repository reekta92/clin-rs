# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.0_beta.6
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.0-beta.6/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("7678141a8574baadc8106c47c43b04ea3f1ac2a33203e80436f11167bd04dfc2")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
