# extract name from package.json
PACKAGE_NAME := bootkitd
RPM_NAME := bootkitd
VERSION := $(shell T=$$(git describe 2>/dev/null) || T=1; echo $$T | tr '-' '.')
VERSION := $(shell cat Cargo.toml | grep version | head -1 | cut -d' ' -f3 | sed 's/"//g')
ifeq ($(TEST_OS),)
TEST_OS = opensuse-tumbleweed
endif
export TEST_OS
TARFILE=$(RPM_NAME)-$(VERSION).tar.xz
# TARFILE=$(RPM_NAME).tar.xz
SPEC=$(RPM_NAME).spec
PREFIX ?= /usr/local
VM_IMAGE=$(CURDIR)/test/images/$(TEST_OS)
# one example file in dist/ from bundler to check if that already ran
DIST_TEST=target/release/bootkit
# make sure we have local db setup for building
DB_TEST=tmp/bootkit.db
# Vendor
VENDOR_TEST=vendor.tar.zst
export DATABASE_URL=sqlite://$(CURDIR)/tmp/bootkit.db
# one example file in pkg/lib to check if it was already checked out
COCKPIT_REPO_STAMP=pkg/lib/cockpit-po-plugin.js
# common arguments for tar, mostly to make the generated tarballs reproducible
TAR_ARGS = --sort=name --mtime "@$(shell git show --no-patch --format='%at')" --mode=go=rX,u+rw,a-s --numeric-owner --owner=0 --group=0

all: $(DIST_TEST)

# checkout common files from Cockpit repository required to build this project;
# this has no API stability guarantee, so check out a stable tag when you start
# a new project, use the latest release, and update it from time to time
COCKPIT_REPO_FILES = \
	pkg/lib \
	test/common \
	$(NULL)

COCKPIT_REPO_URL = https://github.com/cockpit-project/cockpit.git
COCKPIT_REPO_COMMIT = d8e6d902c4d5d972fd057456b5692555db8f98a6 # 364 + 14 commits

$(COCKPIT_REPO_FILES): $(COCKPIT_REPO_STAMP)
COCKPIT_REPO_TREE = '$(strip $(COCKPIT_REPO_COMMIT))^{tree}'
$(COCKPIT_REPO_STAMP): Makefile
	@git rev-list --quiet --objects $(COCKPIT_REPO_TREE) -- 2>/dev/null || \
	    git fetch --no-tags --no-write-fetch-head --depth=1 $(COCKPIT_REPO_URL) $(COCKPIT_REPO_COMMIT)
	git archive $(COCKPIT_REPO_TREE) -- $(COCKPIT_REPO_FILES) | tar x

#
# Build/Install/dist
#

$(SPEC): packaging/$(SPEC).in
	awk '{gsub(/%{VERSION}/, "$(VERSION)");}1' $< > $@

$(DB_TEST):
	./scripts/setup_local_db.sh

$(VENDOR_TEST):
	# TODO: use cargo vendor directly
	/usr/lib/obs/service/cargo_vendor --src "." --update false --outdir $(CURDIR)

$(DIST_TEST): $(COCKPIT_REPO_STAMP) $(DB_TEST) $(shell find src/ -type f)
	cargo build --release

print-version:
	@echo "$(VERSION)"

dist: $(TARFILE)
	@ls -1 $(TARFILE)

$(TARFILE): $(DIST_TEST) $(VENDOR_TEST) $(SPEC)
	tar --xz $(TAR_ARGS) -cf $(TARFILE) --transform 's,^,$(RPM_NAME)-$(VERSION)/,' \
		--exclude packaging/$(SPEC).in --exclude target --exclude tmp \
		$$(git ls-files) $(COCKPIT_REPO_FILES) $(SPEC) vendor.tar.zst

node-cache: $(NODE_CACHE)

# convenience target for developers
srpm: $(TARFILE) $(NODE_CACHE) $(SPEC)
	rpmbuild -bs \
	  --define "_sourcedir `pwd`" \
	  --define "_srcrpmdir `pwd`" \
	  $(SPEC)

# convenience target for developers
rpm: $(TARFILE) $(NODE_CACHE) $(SPEC)
	mkdir -p "`pwd`/output"
	mkdir -p "`pwd`/rpmbuild"
	rpmbuild -bb \
	  --define "_sourcedir `pwd`" \
	  --define "_specdir `pwd`" \
	  --define "_builddir `pwd`/rpmbuild" \
	  --define "_srcrpmdir `pwd`" \
	  --define "_rpmdir `pwd`/output" \
	  --define "_buildrootdir `pwd`/build" \
	  $(SPEC)
	find `pwd`/output -name '*.rpm' -printf '%f\n' -exec mv {} . \;
	rm -r "`pwd`/rpmbuild"
	rm -r "`pwd`/output" "`pwd`/build"

# build a VM with locally built distro pkgs installed
# disable networking, VM images have mock/pbuilder with the common build dependencies pre-installed
$(VM_IMAGE): export XZ_OPT=-0
$(VM_IMAGE): $(TARFILE) bots test/vm.install
	bots/image-customize --verbose --no-network --fresh \
		--upload vendor.tar.zst:/var/tmp --build $(TARFILE) \
		--script $(CURDIR)/test/vm.install $(TEST_OS)

# convenience target for the above
vm: $(VM_IMAGE)
	@echo $(VM_IMAGE)

# convenience target to print the filename of the test image
print-vm:
	@echo $(VM_IMAGE)

# convenience target to setup all the bits needed for the integration tests
# without actually running them
prepare-check: $(NODE_MODULES_TEST) $(VM_IMAGE) test/common

# run the browser integration tests
# this will run all tests/check-* and format them as TAP
check: prepare-check
	test/common/run-tests ${RUN_TESTS_OPTIONS}

codecheck: test/common $(NODE_MODULES_TEST)
	test/common/static-code

# checkout Cockpit's bots for standard test VM images and API to launch them
bots: $(COCKPIT_REPO_STAMP)
	test/setup_bots.sh

.PHONY: all clean install devel-install devel-uninstall print-version dist node-cache rpm prepare-check check vm print-vm
