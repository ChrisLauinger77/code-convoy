#!/bin/sh
set -eu
export LC_ALL=C TZ=UTC SOURCE_DATE_EPOCH=1762560000
export CFLAGS='-Os -fPIC -ffunction-sections -fdata-sections -ffile-prefix-map=/build=. -fdebug-prefix-map=/build=.'
mkdir -p /build /opt/runtime/bin /out
cd /build
tar --no-same-owner -xf /sources/AppImage-runtime-dd6cebed.tar.gz
tar --no-same-owner -xf /sources/musl-1.2.5.tar.gz
cd musl-1.2.5
./configure --prefix=/opt/runtime --disable-shared
make -j4
make install
cd /build
cat > /opt/runtime/bin/cc <<'EOF'
#!/bin/sh
exec gcc -B/opt/runtime/lib -nostdinc -isystem /opt/runtime/include -isystem /usr/lib/gcc/x86_64-alpine-linux-musl/14.2.0/include -idirafter /usr/include -L/opt/runtime/lib "$@"
EOF
chmod +x /opt/runtime/bin/cc
export CC=/opt/runtime/bin/cc
export PKG_CONFIG_LIBDIR=/opt/runtime/lib/pkgconfig
export CPPFLAGS=-I/opt/runtime/include
export LDFLAGS=-L/opt/runtime/lib
# Upstream libfuse mount patch; disable build-only helper binaries.
tar --no-same-owner -xf /sources/libfuse-3.15.0.tar.xz
cd fuse-3.15.0
patch -p1 < /build/type2-runtime-dd6cebedcbddde9c82f89b011e8e1d40b6e43868/patches/libfuse/mount.c.diff
patch -p1 < /kit/libfuse-notice.patch
if test -f /kit/recipient.patch; then patch -p1 < /kit/recipient.patch; fi
meson setup build --prefix=/opt/runtime --libdir=lib --default-library=static --buildtype=release -Dutils=false -Dexamples=false -Dtests=false -Duseroot=false -Ddisable-libc-symbol-version=true
ninja -C build install
cd /build
tar --no-same-owner -xf /sources/zlib-1.3.1.tar.gz
cd zlib-1.3.1
./configure --static --prefix=/opt/runtime
make -j4
make install
cd /build
tar --no-same-owner -xf /sources/zstd-1.5.6.tar.gz
cd zstd-1.5.6
make -j4 -C lib libzstd.a
cp lib/libzstd.a /opt/runtime/lib/
cp lib/zstd.h lib/zstd_errors.h /opt/runtime/include/
cd /build
tar --no-same-owner -xf /sources/mimalloc-2.1.7-source.tar.gz
cmake -S mimalloc-2.1.7 -B mimalloc-build -DCMAKE_INSTALL_PREFIX=/opt/runtime -DMI_BUILD_SHARED=OFF -DMI_BUILD_TESTS=OFF -DMI_BUILD_OBJECT=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build mimalloc-build -j4
cmake --install mimalloc-build
# Place the versioned install archive where the upstream runtime links it.
cp /opt/runtime/lib/mimalloc-2.1/libmimalloc.a /opt/runtime/lib/
cd /build
tar --no-same-owner -xf /sources/squashfuse-0.5.2.tar.gz
cd squashfuse-0.5.2
./autogen.sh
./configure --prefix=/opt/runtime --disable-shared --enable-static --without-xz --without-lzo --without-lz4 --with-zstd=/opt/runtime --with-zlib=/opt/runtime LDFLAGS='-L/opt/runtime/lib -static'
make -j4
make install
mkdir -p /opt/runtime/include/squashfuse
cp ./*.h /opt/runtime/include/squashfuse/
cd /build
cp -R /build/type2-runtime-dd6cebedcbddde9c82f89b011e8e1d40b6e43868 runtime
cd runtime/src/runtime
$CC $CFLAGS -std=gnu99 -D_FILE_OFFSET_BITS=64 '-DGIT_COMMIT="dd6cebed-codeconvoy1"' -I/opt/runtime/include/squashfuse -I/opt/runtime/include/fuse3 -Wall -Werror -Wno-error=format-overflow runtime.c -c -o runtime.o
$CC -static-pie -T data_sections.ld -Wl,--gc-sections,-Map=/out/runtime.map,-t runtime.o -lsquashfuse -lsquashfuse_ll -lzstd -lz -lfuse3 -lmimalloc -o /out/runtime.full > /out/link-inputs.txt
cp /out/runtime.full /out/runtime
strip --strip-debug --strip-unneeded /out/runtime
printf 'AI\002' | dd of=/out/runtime bs=1 count=3 seek=8 conv=notrunc
python3 /kit/build-runtime.py capture /out
sha256sum /out/runtime
