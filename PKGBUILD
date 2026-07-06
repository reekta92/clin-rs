# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.4
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.4/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("f2373ebd311d4f4bc0dbd47d7d8b2b45cb36d15e452ea4525d358b794742b9a7")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
