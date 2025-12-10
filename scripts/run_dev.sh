#!/bin/bash

set -e
cd $(dirname "$0")
cd ..

if [[ ! -d "tmp" ]]; then
    mkdir tmp
    cp /etc/default/grub tmp

    touch tmp/bootkit.db
    sqlite3 tmp/bootkit.db < db/grub2.sql
fi

cargo run --features dev -- --session
