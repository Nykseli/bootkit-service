#!/bin/bash

set -e
cd $(dirname "$0")
cd ..

if [[ ! -e tmp/bootkit.db ]]; then
    mkdir -p tmp
    touch tmp/bootkit.db
    for db_file in $(find db -maxdepth 1 -type f -name '*.sql'); do
        sqlite3 tmp/bootkit.db < "$db_file"
    done
    for db_file in $(find db/migrations -maxdepth 1 -type f -name '*.sql'); do
        sqlite3 tmp/bootkit.db < "$db_file"
    done
fi

echo local db setup complete, add following to env to compile the program
echo export "DATABASE_URL='sqlite://$(pwd)/tmp/bootkit.db'"
