# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.11.0_rc.0
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.11.0-rc.0/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("d97ba109067de685dca672da90dce883283cf4fcb0e83c09b0f491618e06e6bb")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
