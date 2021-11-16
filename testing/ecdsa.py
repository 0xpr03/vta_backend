import jwt
import requests
import uuid
from time import time

pubkey = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEEVs/o5+uQbTjL3chynL4wXgUg2R9\nq9UU8I5mEovUf86QZ7kOBIjJwqnzD1omageEHWwHdBO6B+dFabmdT9POxg==\n-----END PUBLIC KEY-----"
privkey = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2\nOF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r\n1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G\n-----END PRIVATE KEY-----"
payload = {"exp": int( time() ),"sub": "register", "iss": str(uuid.uuid4()),"name": "toaster","delete_after": 1234}
encoded = jwt.encode(payload,key= privkey, algorithm="ES256")
print(encoded)
print(jwt.get_unverified_header(encoded))
# decoded = jwt.decode(encoded, pubkey, algorithms=["ES256"])
# print(decoded)

url = "http://localhost:8080"
header_json = {"Content-Type": "application/json", "Accept" : "application/json"}

payload = {"key": pubkey, "proof": encoded, "keytype": "EC_PEM"}
#payload = {"key": "b", "proof": "c"}
res = requests.post(url+"/api/v1/account/register",json=payload)
print(res)