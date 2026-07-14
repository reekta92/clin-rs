# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.9
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.9/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("5d3c0342d31e0c983858b19097a2632ff07bbef8289ecea54360b94bfdce7189")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
