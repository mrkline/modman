#!/bin/bash

# A crappy set of integration tests.
# Uses a bunch of *nix utils, run accordingly.

set -e

cd test

run='cargo run -q -- '
rootsums='find rootdir -type f -exec sha224sum {} +'
backupsums='find modman-backup -type f -exec sha224sum {} +'

# Make sure that everything's starting the way we expect.
echo "Cleaning up test environment..."
rm -f modman.profile
rm -rf modman-backup

# Make a zip version of mod1
echo "Creating ZIP mod..."
rm -f mod1.zip && sh -c 'cd mod1 && zip -r9 ../mod1.zip *' > /dev/null

echo "Testing init"
$run init --root rootdir
#cp modman.profile expected/empty.profile
$backupsums > expected/empty.backup
diff -u modman.profile expected/empty.profile
diff -u <($backupsums) expected/empty.backup

# A bunch of these rely on the specific error strings.
# That's pretty fragile, but we should be running these tests often enough
# to notice if they get out of sync.

echo "Testing init failure on existing profile"
! $run init --root rootdir 2>&1 | grep -q 'A profile already exists.'

echo "Testing init failure on existing backup directory"
mv modman.profile modman.profile.tmp
! $run init --root rootdir 2>&1 | grep -q "Please move or remove it."
mv modman.profile.tmp modman.profile

echo "Activating mod1"
$run activate mod1.zip

echo "All tests passed!"
