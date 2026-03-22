pkgname=clin
pkgver=0.1.1.1
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
arch=('x86_64' 'aarch64')
url="https://github.com/reekta92/clin-rs"
license=('MIT')
depends=('gcc-libs')
makedepends=('cargo')
source=("clin-${pkgver}.tar.gz::https://github.com/reekta92/clin-rs/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=('SKIP')

build() {
  cd "$srcdir/clin-rs-${pkgver}"
  cargo build --release --locked
}

package() {
  cd "$srcdir/clin-rs-${pkgver}"
  
  install -Dm755 "target/release/clin" "$pkgdir/usr/bin/clin"
  install -Dm644 "clin.ico" "$pkgdir/usr/share/pixmaps/clin.ico"
  install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/${pkgname}/LICENSE"
}
