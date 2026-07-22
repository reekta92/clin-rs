# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_rc.3
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-rc.3/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("c782e9c2bf9427479b690294e7a35372b5a58262375f4935831fdca71ba73495")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
