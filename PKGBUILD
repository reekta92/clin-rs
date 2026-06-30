# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.1
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.1/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("6b87e90570e80f247ee0c88aa505e0339ed7c4db96aa1c744e1bf40ed57b9d5c")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
