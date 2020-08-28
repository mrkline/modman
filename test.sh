#!/bin/bash

# A crappy set of integration tests.
# Uses a bunch of *nix utils, run accordingly.

set -euo pipefail

cd test

run='cargo run -q -- -vvv'
quietrun='cargo run -q --'

rootsums()
{
    find rootdir -type f | LC_ALL=C sort | tr '\n' '\0' | xargs -0 sha224sum
}

backupsums()
{
   find modman-backup -type f | LC_ALL=C sort | tr '\n' '\0' | xargs -0 sha224sum
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

echo "Testing repair"

#rootsums > expected/starting.root

# Set things up as if `add mod1.zip` was interrupted right before
# updating the profile.
mv rootdir/A.txt modman-backup/originals/A.txt
mv rootdir/B.txt modman-backup/originals/B.txt
cp mod1/modroot/A.txt rootdir
cp mod1/modroot/B.txt rootdir
cp mod1/modroot/C.txt rootdir
echo "Replace A.txt" > modman-backup/temp/activate.journal
echo "Replace B.txt" >> modman-backup/temp/activate.journal
echo "Add C.txt" >> modman-backup/temp/activate.journal

$run repair
diff -u <(rootsums) expected/starting.root
diff -u <(backupsums) expected/empty.backup

echo "Activating a ZIP mod (mod1)"
$run add mod1.zip
#cp modman.profile expected/mod1.profile
#backupsums > expected/mod1.backup
#rootsums > expected/mod1.root
diff -u modman.profile expected/mod1.profile
diff -u expected/mod1.backup <(backupsums)
diff -u expected/mod1.root <(rootsums)

echo "Activating a directory mod (mod2)"
$run add mod2
#cp modman.profile expected/mod2.profile
#backupsums > expected/mod2.backup
#rootsums > expected/mod2.root
diff -u modman.profile expected/mod2.profile
diff -u expected/mod2.backup <(backupsums)
diff -u expected/mod2.root <(rootsums)

echo "Testing activation failure when adding the same mod twice"
out=$(! $run add mod1.zip 2>&1)
echo "$out" | grep -q "mod1.zip has already been added!"

echo "Testing activation conflict detection"
out=$(! $run add mod-conflicting.zip 2>&1)
echo "$out" | grep -q "A.txt from mod-conflicting.zip would overwrite the same file from mod1"

echo "Testing list"
#$run list -f -r > expected/list.txt
diff -u expected/list.txt <($run list --files --readme)

echo "Testing check"
$run check
# Mess with the backup files, the game files,
# and create a fake journal
touch modman-backup/temp/activate.journal
mv modman-backup/originals/A.txt modman-backup/originals/wut.txt
echo "Changed backup contents" > modman-backup/originals/A.txt
echo "Changed game contents" > rootdir/A.txt
#! $quietrun -- check > expected/check.warns 2>&1
out=$(! $quietrun check 2>&1)
diff -u expected/check.warns <(echo "$out")
# Undo those changes.
rm modman-backup/temp/activate.journal
mv modman-backup/originals/wut.txt modman-backup/originals/A.txt
cp mod1/modroot/A.txt rootdir/A.txt
$run check

echo "Testing update with version mismatch"
echo "1.2.3" > mod2/VERSION.txt
out=$(! $run update 2>&1)
echo "$out" | grep -q "mod2's version ([1-9.]\+) doesn't match what it was"
git checkout -- mod2/VERSION.txt

echo "Testing no-op update"
$run update
diff -u modman.profile expected/mod2.profile
diff -u expected/mod2.backup <(backupsums)
diff -u expected/mod2.root <(rootsums)

echo "Testing update"
echo "I am the latest and greatest version of B." > rootdir/B.txt
echo "I am a new game file replacing the mod file, C." > rootdir/C.txt
$run update
backupsums > expected/updated.backup
rootsums > expected/updated.root
diff -u expected/updated.backup <(backupsums)
diff -u expected/updated.root <(rootsums)

echo "Testing remove"
$run remove mod1.zip mod2
diff -u modman.profile expected/empty.profile
diff -u expected/empty.backup <(backupsums)
# We expect the "updates" applied above to persist through removal.
diff -u <(echo "I am the latest and greatest version of B.") rootdir/B.txt
diff -u <(echo "I am a new game file replacing the mod file, C.") rootdir/C.txt
# Then reset them to how they started
git checkout -- rootdir/B.txt
rm rootdir/C.txt

echo "All tests passed!"
