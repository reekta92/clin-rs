# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_rc.1
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-rc.1/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("2c8a94c6a51c3bea344f0ebfb7bea212bfcd56a73ffc198c3e3a2ef476dbd8ee")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
