NEW_MALTER="/home/pi/malter-bin/malter-$(git rev-parse --short HEAD)"
MAIN_MAITER="/home/pi/malter-bin/malter"

cp "target/release/malter" $NEW_MALTER
rm $MAIN_MAITER
ln $NEW_MALTER $MAIN_MAITER
