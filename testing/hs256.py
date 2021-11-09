import jwt

key = "secret"
encoded = jwt.encode({"some": "payload"}, key, algorithm="HS256")
print(encoded)
print(jwt.decode(encoded, key, algorithms="HS256"))