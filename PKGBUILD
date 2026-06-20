# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.8.23
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v${pkgver}/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("b98caab6c0427982b1a114f87305839c4ab633bc841f53c9dc93a2744e7ca938")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
