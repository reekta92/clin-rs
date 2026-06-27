# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.8.32
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.8.32/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("d533bfbf0394d0976a1a8f6604341e1a45ec27470c8b5806491a0918aded325b")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
