# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.0_beta.5
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.0-beta.5/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("a3175565b3279b504945afaf0db0146ce6a1efac72558257b0e473b459bbe13f")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
