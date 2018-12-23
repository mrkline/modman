#!/bin/bash

# A crappy set of integration tests.
# Uses a bunch of *nix utils, run accordingly.

set -e

cd test

run='cargo run -q -- '

rootsums()
{
    find rootdir -type f | sort | tr '\n' '\0' | xargs -0 sha224sum
}

backupsums()
{
   find modman-backup -type f | sort | tr '\n' '\0' | xargs -0 sha224sum
}

echo "Building..."
cargo build

# Make sure that everything's starting the way we expect.
echo "Cleaning up test environment..."
rm -f modman.profile
rm -rf modman-backup

# Make a zip version of mod1
echo "Creating ZIP mods..."
rm -f mod1.zip && sh -c 'cd mod1 && zip -r9 ../mod1.zip *' > /dev/null
rm -f mod-conflicting.zip && sh -c 'cd mod-conflicting && zip -r9 ../mod-conflicting.zip *' > /dev/null

echo "Testing init"
$run init --root rootdir
#cp modman.profile expected/empty.profile
#backupsums > expected/empty.backup
diff -u modman.profile expected/empty.profile
diff -u <(backupsums) expected/empty.backup

# A bunch of these rely on the specific error strings.
# That's pretty fragile, but we should be running these tests often enough
# to notice if they get out of sync.

echo "Testing init failure on existing profile"
out=$(! $run init --root rootdir 2>&1)
echo "$out" | grep -q 'A profile already exists.'

echo "Testing init failure on existing backup directory"
mv modman.profile modman.profile.tmp
out=$(! $run init --root rootdir 2>&1)
echo "$out" | grep -q "Please move or remove it."
mv modman.profile.tmp modman.profile

echo "Activating mod1"
$run activate mod1.zip
#cp modman.profile expected/mod1.profile
#backupsums > expected/mod1.backup
#rootsums > expected/mod1.root
diff -u modman.profile expected/mod1.profile
diff -u expected/mod1.backup <(backupsums)
diff -u expected/mod1.root <(rootsums)

echo "Testing activation failure when adding the same mod twice"
out=$(! $run activate mod1.zip 2>&1)
echo "$out" | grep -q "mod1.zip has already been activated!"

echo "Testing activation conflict detection"
out=$(! $run activate mod-conflicting.zip 2>&1)
echo "$out" | grep -q "A.txt from mod-conflicting.zip would overwrite the same file from mod1.zip"

echo "Testing check"
$run check
# Mess with the backup files, the game files,
# and create a fake journal
touch modman-backup/temp/activate.journal
mv modman-backup/originals/A.txt modman-backup/originals/wut.txt
echo "Changed backup contents" > modman-backup/originals/A.txt
echo "Changed game contents" > rootdir/A.txt
#! $run check > expected/check.warns 2>&1
out=$(! $run check 2>&1)
diff -u expected/check.warns <(echo "$out")
# Undo those changes.
rm modman-backup/temp/activate.journal
mv modman-backup/originals/wut.txt modman-backup/originals/A.txt
cp mod1/modroot/A.txt rootdir/A.txt
$run check


## TODO: deactivate. For now, do it manually, with shell
rm -r rootdir/*
mv modman-backup/originals/* rootdir
cp expected/empty.profile modman.profile
# Will actually be meaningful once deactivate is done.
diff -u modman.profile expected/empty.profile
diff -u expected/empty.backup <(backupsums)

echo "All tests passed!"
