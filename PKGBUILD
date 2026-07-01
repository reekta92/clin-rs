# Maintainer: reekta92 mdag.92988@protonmail.com
pkgname=clin-rs-bin
pkgver=0.9.2
pkgrel=1
pkgdesc="Feature-packed terminal note management app"
url="https://github.com/reekta92/clin-rs"
license=("GPL-3.0")
arch=("x86_64")
provides=("clin-rs" "clin")
conflicts=("clin-rs")
depends=("openssl" "gcc-libs")
source=("https://github.com/reekta92/clin-rs/releases/download/v0.9.2/clin-rs-x86_64-unknown-linux-gnu.tar.xz")
sha256sums=("2bfd380a1410d7d7b15ede864e2e7cdfc80edbbe73d45e58690667fea940ced3")

package() {
    install -Dm755 "clin" -t "$pkgdir/usr/bin"
}
