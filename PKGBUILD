# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.8
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.8/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("62187b2debe2df05a38a966d3e4364a010f2e24426d34da5edc728c701890647")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
