# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.8.17
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v${pkgver}/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("1ebb7a4dd3647dae62385ac02af6dabb05290cfb9f87794adeae907d23df3d5d")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
