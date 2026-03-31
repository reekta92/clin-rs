pkgname=clin-git
pkgver=0.1.1.1
pkgrel=1
pkgdesc="Encrypted terminal note-taking app"
arch=('x86_64' 'aarch64')
url="https://github.com/reekta92/clin-rs"
license=('MIT')
depends=('gcc-libs' 'glibc')
makedepends=('cargo' 'git')
provides=('clin')
conflicts=('clin')
source=("clin-rs::git+https://github.com/reekta92/clin-rs.git")
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/clin-rs"
  git describe --long --tags | sed 's/^v//;s/\([^-]*-g\)/r\1/;s/-/./g'
}

build() {
  cd "$srcdir/clin-rs"
  cargo build --release --locked
}

package() {
  cd "$srcdir/clin-rs"
  install -Dm755 "target/release/clin" "$pkgdir/usr/bin/clin"
  install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/${pkgname}/LICENSE"
}
