# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.8.25
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v${pkgver}/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("bc3f065e64ccb5328eea5f6006a0037f2df129f10b17e219cf54ec4bb4e6ed3f")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
