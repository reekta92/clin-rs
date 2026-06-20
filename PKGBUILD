# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.8.21
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v${pkgver}/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("997ce214fcd10305b9ef8ada815ef8b4384ebfb7627bf5d9e6692b5857f33c8c")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
