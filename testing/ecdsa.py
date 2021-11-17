import jwt
import requests
import uuid
from time import time

url = "http://localhost:8080"
#header_json = {"Content-Type": "application/json", "Accept" : "application/json"}


server_info = requests.get(url+"/api/v1/server/info").json()
print(server_info)

pubkey = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEEVs/o5+uQbTjL3chynL4wXgUg2R9\nq9UU8I5mEovUf86QZ7kOBIjJwqnzD1omageEHWwHdBO6B+dFabmdT9POxg==\n-----END PUBLIC KEY-----"
privkey = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2\nOF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r\n1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G\n-----END PRIVATE KEY-----"
payload = {
    #"aud": [server_info["id"]],
    "aud": ["612b4cd3-87c4-4c81-abdd-f7cecadbcef0"],
    "nbf": int( time()-4 ),
    "iat": int( time() ),
    "exp": int( time()+5 ),
    "sub": "register",
    "iss": str(uuid.uuid4()),
    "name": "toaster",
    "delete_after": int( time()+60*60 )
    }
print("send payload: "+str(payload))
encoded = jwt.encode(payload,key= privkey, algorithm="ES256")
print("encoded: "+str(encoded))
print("send header: "+str(jwt.get_unverified_header(encoded)))
# decoded = jwt.decode(encoded, pubkey, algorithms=["ES256"])
# print(decoded)


payload = {"key": pubkey, "proof": encoded, "keytype": "EC_PEM"}
res = requests.post(url+"/api/v1/account/register",json=payload)
print(res)
print(res.url)
print(res.text)
print(res.headers)