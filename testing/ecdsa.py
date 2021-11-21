from typing import AnyStr, Dict
import jwt
import requests
import uuid
from time import time

url = "http://127.0.0.1:8080"
#header_json = {"Content-Type": "application/json", "Accept" : "application/json"}



pubkey = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEEVs/o5+uQbTjL3chynL4wXgUg2R9\nq9UU8I5mEovUf86QZ7kOBIjJwqnzD1omageEHWwHdBO6B+dFabmdT9POxg==\n-----END PUBLIC KEY-----"
privkey = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2\nOF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r\n1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G\n-----END PRIVATE KEY-----"

def server_info():
    return requests.get(url+"/api/v1/server/info").json()

server = server_info()
user_id = uuid.uuid4()

def jwt_payload(server,user_id,sub) -> Dict[str, AnyStr]:
    return {
        "aud": [server["id"]],
        #"aud": ["612b4cd3-87c4-4c81-abdd-f7cecadbcef0"],
        "nbf": int( time()-4 ),
        "iat": int( time() ),
        "exp": int( time()+5 ),
        "sub": sub,
        "iss": str(user_id),
        "name": "toaster",
        "delete_after": int( time()+60*60 )
        }

def register(server, pubkey, privkey, user_id):
    payload = jwt_payload(server,user_id, "register")
    payload["name"] = "toaster"
    payload["delete_after"] = int( time()+60*60 )
    print("send payload: "+str(payload))
    encoded = jwt.encode(payload,key= privkey, algorithm="ES256")
    print("encoded: "+str(encoded))
    print("send header: "+str(jwt.get_unverified_header(encoded)))

    payload = {"key": pubkey, "proof": encoded, "keytype": "EC_PEM"}
    res = requests.post(url+"/api/v1/account/register/new",json=payload)
    if res.status_code != 202:
        print(res)
        print(res.url)
        print(res.text)
        print(res.headers)
        raise Exception(f"Register failed with {res.status_code}")

def login(server, privkey, user_id):
    curSession = requests.Session()
    payload = jwt_payload(server,user_id, "login")
    encoded = jwt.encode(payload,key= privkey, algorithm="ES256")

    payload = {"iss": str(user_id), "proof": encoded}
    res = curSession.post(url+"/api/v1/account/login/key",json=payload)
    print(res)
    print(res.url)
    print(res.text)
    print(res.headers)
    if res.status_code != 204:
        raise Exception(f"Login failed with {res.status_code}")
    return curSession

def account_info(session):
    res = session.get(url+"/api/v1/account/info")
    if res.status_code != 200:
        print(res)
        print(res.text)
        raise Exception(f"User info failed with {res.status_code}")
    return res.json()


startTime = time()
print("registering")
register(server, pubkey, privkey, user_id)
print("logging in")
session = login(server, privkey, user_id)
print(session)
print(account_info(session))
executionTime = (time() - startTime)
print('Execution time in seconds: ' + str(executionTime))