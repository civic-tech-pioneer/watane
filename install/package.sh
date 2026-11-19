#!/bin/sh
# package.sh: assembles one release archive from built programs (install/TECHNICAL 4.2, 5).
# usage: package.sh <target> <version> <out-dir>
# The inputs lie in the checkout that holds this script: target/release/{watane,watane-dev,
# watane-node}, target/release/bundle/macos/watane.app, dev/scripts/vendor.sh, install/install.sh.
set -eu
umask 022

die() {
  printf 'package.sh: %s\n' "$*" >&2
  exit 1
}

[ "$#" -eq 3 ] || die "usage: package.sh <target> <version> <out-dir>"
target=$1
version=$2
out=$3
case $version in '' | *[!0-9A-Za-z.+-]*) die "not a version: $version" ;; esac
case $target in '' | *[!0-9A-Za-z_-]*) die "not a target: $target" ;; esac

here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH='' cd -- "$here/.." && pwd)
rel=$root/target/release
app=$rel/bundle/macos/watane.app
vendor_sh=$root/dev/scripts/vendor.sh
script=$here/install.sh
top=watane-$version-$target

for p in watane watane-dev watane-node; do
  if [ ! -f "$rel/$p" ] || [ ! -x "$rel/$p" ]; then
    die "missing part: the program $rel/$p"
  fi
done
[ -d "$app" ] || die "missing part: the app $app"
[ -f "$app/Contents/Info.plist" ] || die "missing part: $app/Contents/Info.plist"
[ -f "$vendor_sh" ] || die "missing part: the vendor script $vendor_sh"
[ -f "$script" ] || die "missing part: $script"
grep -q 'DEFAULT_RELEASE="@RELEASE@"' "$script" || die "$script has no @RELEASE@ to replace"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
t=$work/$top
mkdir -p "$t/bin" "$t/share/watane" "$t/Applications"

sh "$vendor_sh" "$work/vendor" >&2 || die "missing part: the vendor folder ($vendor_sh failed)"
if [ ! -d "$work/vendor" ] || [ -z "$(ls -A "$work/vendor")" ]; then
  die "missing part: the vendor folder is empty"
fi
mkdir -p "$t/share/watane-dev"
cp -R "$work/vendor" "$t/share/watane-dev/vendor"

for p in watane watane-dev watane-node; do
  cp "$rel/$p" "$t/bin/$p"
  chmod 755 "$t/bin/$p"
done
cp -R "$app" "$t/Applications/watane.app"
printf '%s\n' "$version" >"$t/VERSION"
printf '%s\n' "$version" >"$t/share/watane/VERSION"
sed "s/DEFAULT_RELEASE=\"@RELEASE@\"/DEFAULT_RELEASE=\"$version\"/" "$script" >"$t/share/watane/install.sh"
chmod 755 "$t/share/watane/install.sh"

# ad hoc signatures: Apple silicon will not run an unsigned binary (no identity, 6)
if command -v codesign >/dev/null 2>&1; then
  for p in watane watane-dev watane-node; do
    codesign --force --sign - "$t/bin/$p" || die "codesign failed on $p"
  done
  codesign --force --deep --sign - "$t/Applications/watane.app" || die "codesign failed on watane.app"
  for p in "$t/bin/watane" "$t/bin/watane-dev" "$t/bin/watane-node" "$t/Applications/watane.app"; do
    codesign --verify --deep --strict "$p" || die "signature check failed on $p"
  done
fi
if command -v otool >/dev/null 2>&1; then
  for p in watane watane-dev watane-node; do
    otool -L "$t/bin/$p" >/dev/null || die "otool failed on $p"
  done
fi

mkdir -p "$out"
archive=$top.tar.gz
COPYFILE_DISABLE=1 tar -czf "$work/$archive" -C "$work" "$top" || die "tar failed"
# install.sh refuses an archive with a link member (3.4 step 9): never make one
if tar -tvzf "$work/$archive" | grep -q '^[lh]'; then
  die "an input holds a symbolic or hard link: the archive would be refused by install.sh"
fi
mv "$work/$archive" "$out/$archive"
cp "$t/share/watane/install.sh" "$out/install.sh"
if command -v shasum >/dev/null 2>&1; then
  (cd "$out" && shasum -a 256 "$archive" >SHA256SUMS)
else
  (cd "$out" && sha256sum "$archive" >SHA256SUMS)
fi
printf '%s\n' "$out/$archive"
