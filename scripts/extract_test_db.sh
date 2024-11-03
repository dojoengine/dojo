# Ensures that the user has locally the db dir in /tmp.

rm -rf /tmp/spawn-and-move-db
rm -rf /tmp/types-test-db
tar xzf spawn-and-move-db.tar.gz -C /tmp/
tar xzf types-test-db.tar.gz -C /tmp/
