# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.13.1
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.13.1/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("9b27e24dd0f2361e00fc60891f826b2129c546db7a6b45ed02693dd5a7603044")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
