# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.10.0_beta.3
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.10.0-beta.3/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("4884e7100cb18023604d5e14c074bbd946c0aee9583d796f9f2e670d2aecbbe7")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
