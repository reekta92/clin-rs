# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_rc.2
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-rc.2/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("df214c96a692dd89bcc5cc8fa38391dd0260aba46a50b29ce9017276e961e4ff")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
