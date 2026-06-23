# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.8.31
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.8.31/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("d0d43fdb743022c1e80d03f985ff948e43853bf73976ef491652645bef3b3403")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
