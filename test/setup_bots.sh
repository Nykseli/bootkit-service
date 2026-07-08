#!/bin/bash

set -eu

cd "$(dirname ${BASH_SOURCE[0]})"
cd ..

# TODO: get bots if doesn't already exist (here or make file?)
# call this in `make bots`

if [ -e bots ]; then
    echo "bots/ already exists, skipping"
    exit 0
fi


git clone https://github.com/Nykseli/cockpit-bots/ bots
cd bots
git checkout bootkitd
./image-create -v opensuse-tumbleweed
./image-create -v opensuse-tumbleweed-efi
