# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.8.9
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v${pkgver}/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("1d6932b81d0403e9a0ade54ebd35f26d2cfdfc59ad640d19e97cd853a3856bb5")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
