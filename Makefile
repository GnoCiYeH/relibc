# TARGET?=$(shell rustc -Z unstable-options --print target-spec-json | grep llvm-target | cut -d '"' -f4)
TARGET=x86_64-unknown-dragonos

CARGO?=cargo
CARGO_TEST?=$(CARGO)
CARGO_COMMON_FLAGS=-Z build-std=core,alloc,compiler_builtins
CARGOFLAGS?=$(CARGO_COMMON_FLAGS)
RUSTCFLAGS?=
export OBJCOPY?=objcopy

BUILD="target/$(TARGET)"
CARGOFLAGS+="--target=$(TARGET)"

ifeq ($(TARGET),aarch64-unknown-linux-gnu)
	export CC=aarch64-linux-gnu-gcc
	export LD=aarch64-linux-gnu-ld
	export AR=aarch64-linux-gnu-ar
	export OBJCOPY=aarch64-linux-gnu-objcopy
endif

ifeq ($(TARGET),aarch64-unknown-redox)
	export CC=aarch64-unknown-redox-gcc
	export LD=aarch64-unknown-redox-ld
	export AR=aarch64-unknown-redox-ar
	export OBJCOPY=aarch64-unknown-redox-objcopy
endif

ifeq ($(TARGET),x86_64-unknown-linux-gnu)
	export CC=x86_64-linux-gnu-gcc
	export LD=x86_64-linux-gnu-ld
	export AR=x86_64-linux-gnu-ar
	export OBJCOPY=x86_64-linux-gnu-objcopy
endif

ifeq ($(TARGET),i686-unknown-redox)
	export CC=i686-unknown-redox-gcc
	export LD=i686-unknown-redox-ld
	export AR=i686-unknown-redox-ar
	export OBJCOPY=i686-unknown-redox-objcopy
endif

ifeq ($(TARGET),x86_64-unknown-redox)
	export CC=x86_64-unknown-redox-gcc
	export LD=x86_64-unknown-redox-ld
	export AR=x86_64-unknown-redox-ar
	export OBJCOPY=x86_64-unknown-redox-objcopy
endif

ifeq ($(TARGET),x86_64-unknown-dragonos)
# 如果存在x86_64-dragonos-gcc，就用它，否则用gcc

ifeq ($(shell which x86_64-dragonos-gcc),)
	export CC=gcc
# export LD=ld
	export LD=ld
	export AR=ar
# export AR=x86_64-dragonos-ar
	export OBJCOPY=objcopy
# export OBJCOPY=x86_64-dragonos-objcopy
else
	export CC=x86_64-dragonos-gcc
	export LD=x86_64-dragonos-ld
	export AR=x86_64-dragonos-ar
	export OBJCOPY=x86_64-dragonos-objcopy
endif
endif

SRC=\
	Cargo.* \
	$(shell find src -type f)

BUILTINS_VERSION=0.1.70

.PHONY: all clean fmt install install-headers libs submodules test

all: | libs

clean:
	$(CARGO) clean
	$(MAKE) -C tests clean
	rm -rf sysroot

check:
	$(CARGO) check

fmt:
	./fmt.sh

install-headers: libs
	mkdir -pv "$(DESTDIR)/include"
	cp -rv "include"/* "$(DESTDIR)/include"
	cp -rv "target/include"/* "$(DESTDIR)/include"
	cp -v "openlibm/include"/*.h "$(DESTDIR)/include"
	cp -v "openlibm/src"/*.h "$(DESTDIR)/include"
	cp -v "pthreads-emb/"*.h "$(DESTDIR)/include"

# $(BUILD)/release/libc.so
libs: \
	$(BUILD)/release/libc.a \
	$(BUILD)/release/crt0.o \
	$(BUILD)/release/crti.o \
	$(BUILD)/release/crtn.o
#$(BUILD)/release/ld_so

install-libs: libs
	mkdir -pv "$(DESTDIR)/lib"
	cp -v "$(BUILD)/release/libc.a" "$(DESTDIR)/lib"
# cp -v "$(BUILD)/release/libc.so" "$(DESTDIR)/lib"
# ln -frsv "$(DESTDIR)/lib/libc.so" "$(DESTDIR)/lib/libc.so.6"
	cp -v "$(BUILD)/release/crt0.o" "$(DESTDIR)/lib"
	ln -frsv "$(DESTDIR)/lib/crt0.o" "$(DESTDIR)/lib/crt1.o"
	cp -v "$(BUILD)/release/crti.o" "$(DESTDIR)/lib"
	cp -v "$(BUILD)/release/crtn.o" "$(DESTDIR)/lib"
# cp -v "$(BUILD)/release/ld_so" "$(DESTDIR)/lib/ld64.so.1"
	cp -v "$(BUILD)/openlibm/libopenlibm.a" "$(DESTDIR)/lib/libm.a"
	cp -v "$(BUILD)/pthreads-emb/libpthread.a" "$(DESTDIR)/lib/libpthread.a"
	# Empty libraries for dl and rt
	$(AR) -rcs "$(DESTDIR)/lib/libdl.a"
	$(AR) -rcs "$(DESTDIR)/lib/librt.a"

install: install-headers install-libs

submodules:
	git submodule sync
	git submodule update --init --recursive

sysroot: all
	rm -rf $@
	rm -rf $@.partial
	mkdir -p $@.partial
	$(MAKE) install DESTDIR=$@.partial
	mv $@.partial $@
	touch $@

test: sysroot
	# TODO: Fix SIGILL when running cargo test
	# $(CARGO_TEST) test
	$(MAKE) -C tests verify

# Debug targets

$(BUILD)/debug/libc.a: $(BUILD)/debug/librelibc.a $(BUILD)/pthreads-emb/libpthread.a $(BUILD)/openlibm/libopenlibm.a
	echo "create $@" > "$@.mri"
	for lib in $^; do\
		echo "addlib $$lib" >> "$@.mri"; \
	done
	echo "save" >> "$@.mri"
	echo "end" >> "$@.mri"
	$(AR) -M < "$@.mri"

$(BUILD)/debug/libc.so: $(BUILD)/debug/librelibc.a $(BUILD)/pthreads-emb/libpthread.a $(BUILD)/openlibm/libopenlibm.a
	$(CC) -nostdlib -shared -Wl,--allow-multiple-definition -Wl,--whole-archive $^ -Wl,--no-whole-archive -Wl,-soname,libc.so.6 -o $@

$(BUILD)/debug/librelibc.a: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc $(CARGOFLAGS) -- --emit link=$@ $(RUSTCFLAGS)
	./renamesyms.sh $@ $(BUILD)/debug/deps/
	touch $@

$(BUILD)/debug/crt0.o: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --manifest-path src/crt0/Cargo.toml $(CARGOFLAGS) -- --emit obj=$@ -C panic=abort $(RUSTCFLAGS)
	touch $@

$(BUILD)/debug/crti.o: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --manifest-path src/crti/Cargo.toml $(CARGOFLAGS) -- --emit obj=$@ -C panic=abort $(RUSTCFLAGS)
	touch $@

$(BUILD)/debug/crtn.o: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --manifest-path src/crtn/Cargo.toml $(CARGOFLAGS) -- --emit obj=$@ -C panic=abort $(RUSTCFLAGS)
	touch $@

$(BUILD)/debug/ld_so.o: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --manifest-path src/ld_so/Cargo.toml $(CARGOFLAGS) -- --emit obj=$@ -C panic=abort $(RUSTCFLAGS)
	touch $@

$(BUILD)/debug/ld_so: $(BUILD)/debug/ld_so.o $(BUILD)/debug/crti.o $(BUILD)/debug/libc.a $(BUILD)/debug/crtn.o
	$(LD) --no-relax -T src/ld_so/ld_script/$(TARGET).ld --allow-multiple-definition --gc-sections --gc-keep-exported $^ -o $@

# Release targets

$(BUILD)/release/libc.a: $(BUILD)/release/librelibc.a $(BUILD)/pthreads-emb/libpthread.a $(BUILD)/openlibm/libopenlibm.a
	echo "create $@" > "$@.mri"
	for lib in $^; do\
		echo "addlib $$lib" >> "$@.mri"; \
	done
	echo "save" >> "$@.mri"
	echo "end" >> "$@.mri"
	$(AR) -M < "$@.mri"

$(BUILD)/release/libc.so: $(BUILD)/release/librelibc.a $(BUILD)/pthreads-emb/libpthread.a $(BUILD)/openlibm/libopenlibm.a
	$(CC) -nostdlib -shared -Wl,--allow-multiple-definition -Wl,--whole-archive $^ -Wl,--no-whole-archive -Wl,-soname,libc.so.6 -o $@

$(BUILD)/release/librelibc.a: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --release $(CARGOFLAGS) -- --emit link=$@ $(RUSTCFLAGS)
	# TODO: Better to only allow a certain whitelisted set of symbols? Perhaps
	# use some cbindgen hook, specify them manually, or grep for #[no_mangle].
	./renamesyms.sh $@ $(BUILD)/release/deps/
	touch $@

$(BUILD)/release/crt0.o: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --release --manifest-path src/crt0/Cargo.toml $(CARGOFLAGS) -- --emit obj=$@ -C panic=abort $(RUSTCFLAGS)
	touch $@

$(BUILD)/release/crti.o: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --release --manifest-path src/crti/Cargo.toml $(CARGOFLAGS) -- --emit obj=$@ -C panic=abort $(RUSTCFLAGS)
	touch $@

$(BUILD)/release/crtn.o: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --release --manifest-path src/crtn/Cargo.toml $(CARGOFLAGS) -- --emit obj=$@ -C panic=abort $(RUSTCFLAGS)
	touch $@

$(BUILD)/release/ld_so.o: $(SRC)
	CARGO_INCREMENTAL=0 $(CARGO) rustc --release --manifest-path src/ld_so/Cargo.toml $(CARGOFLAGS) -- --emit obj=$@ -C panic=abort $(RUSTCFLAGS)
	touch $@

$(BUILD)/release/ld_so: $(BUILD)/release/ld_so.o $(BUILD)/release/crti.o $(BUILD)/release/libc.a $(BUILD)/release/crtn.o
	$(LD) --no-relax -T src/ld_so/ld_script/$(TARGET).ld --allow-multiple-definition --gc-sections --gc-keep-exported $^ -o $@

# Other targets

$(BUILD)/openlibm: openlibm
	rm -rf $@ $@.partial
	mkdir -p $(BUILD)
	cp -r $< $@.partial
	mv $@.partial $@
	touch $@

$(BUILD)/openlibm/libopenlibm.a: $(BUILD)/openlibm $(BUILD)/release/librelibc.a
	$(MAKE) AR=$(AR) CC=$(CC) LD=$(LD) CPPFLAGS="-fno-stack-protector -I $(shell pwd)/include -I $(shell pwd)/target/include" -C $< libopenlibm.a

$(BUILD)/pthreads-emb: pthreads-emb
	rm -rf $@ $@.partial
	mkdir -p $(BUILD)
	cp -r $< $@.partial
	mv $@.partial $@
	touch $@

$(BUILD)/pthreads-emb/libpthread.a: $(BUILD)/pthreads-emb $(BUILD)/release/librelibc.a
	$(MAKE) AR=$(AR) CC=$(CC) LD=$(LD) CFLAGS="-fno-stack-protector -I $(shell pwd)/include -I $(shell pwd)/target/include" -C $< libpthread.a
