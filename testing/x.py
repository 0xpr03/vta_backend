import mariadb
import sys
from urllib.parse import urlparse
import os

# CI database setup script
# otherwise we would need to
# a) provide the sqlx-cli tool for migrations or
# b) compile without database checking to run migrations first

#DATABASE_URL="mysql://root@localhost/vta_sync"
db_conn_url = urlparse(os.environ['DATABASE_URL'])
port = 3306
if db_conn_url.port is not None:
    port = db_conn_url.port
print(f"{port} {db_conn_url.hostname}")
try:
    conn = mariadb.connect(
        user=db_conn_url.username,
        password=db_conn_url.password,
        host=db_conn_url.hostname,
        port=port,
        database=db_conn_url.path.strip("/")
    )
except mariadb.Error as e:
    print(f"Error connecting to MariaDB Platform: {e}")
    sys.exit(1)
print("connected to db")
cur = conn.cursor()
cur.execute("SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA")
print(f"found DBs: {cur.fetchall()}")
for file in sorted(os.listdir("migrations")):
    f = open("migrations/"+file,"r")
    lines = f.read().split(";")
    for sql in lines:
        if sql != "":
            try:
                cur.execute(sql)
            except mariadb.Error as e:
                print(f"Error setting up {sql}: {e}")
                sys.exit(1)

print("finished setting up db")