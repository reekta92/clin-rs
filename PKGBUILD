# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.0_rc.2
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.0-rc.2/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("642a74cff3264ab533d1edf29822bdb1e5af805a4b95b5b652e38920b6a3f480")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
