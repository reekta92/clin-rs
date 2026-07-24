# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_rc.5
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-rc.5/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("b5d2006515ebcd1290b2fc4093391bfddb63f1ffe3d49919520be43a34703ea9")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
