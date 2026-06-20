# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.8.20
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v${pkgver}/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("610c49ec5702cd49fcba8e91ebc1fb8f411a098e9b7a35adb8399132b91b4fb9")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
