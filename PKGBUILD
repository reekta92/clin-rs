# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.0
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.0/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("ab44f27fd565c436b545bd8b2f49aefa72cfb7c25a7b9cdd38625c834e21fc1f")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
