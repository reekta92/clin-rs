# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_rc.0
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-rc.0/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("aa44d301a828525eed2a4af094680219603c4cef4c17f06ad940eb5e2f7e63b3")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
