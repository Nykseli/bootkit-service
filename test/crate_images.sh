#!/bin/bash

set -eu

cd "$(dirname ${BASH_SOURCE[0]})"
cd ..

export TEST_OS="opensuse-tumbleweed"
make vm
export TEST_OS="opensuse-tumbleweed-efi"
make vm
