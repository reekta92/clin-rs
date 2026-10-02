# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.13.2
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.13.2/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("a46e19633bf7d5b868c32d7f9e7d8981a3216993ecf6eaf497c117bbbe1bb321")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
