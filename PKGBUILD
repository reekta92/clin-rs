# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_rc.7
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-rc.7/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("6e9d596fbafbcb019bd815d8b79817033cbc82dc28b33c0d13a08c507eeba8bd")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
