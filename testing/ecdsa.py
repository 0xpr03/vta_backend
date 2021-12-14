from typing import AnyStr, Dict
import jwt
import requests
import uuid
from time import time
import random
import datetime

url = "http://127.0.0.1:8080"
#header_json = {"Content-Type": "application/json", "Accept" : "application/json"}



pubkey = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEEVs/o5+uQbTjL3chynL4wXgUg2R9\nq9UU8I5mEovUf86QZ7kOBIjJwqnzD1omageEHWwHdBO6B+dFabmdT9POxg==\n-----END PUBLIC KEY-----"
privkey = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2\nOF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r\n1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G\n-----END PRIVATE KEY-----"

def server_info():
    return requests.get(url+"/api/v1/server/info").json()

def generate_random_string(len_sep, no_of_blocks):
    random_string = ''
    random_str_seq = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
    for i in range(0,len_sep*no_of_blocks):
        if i % len_sep == 0 and i != 0:
            random_string += '-'
        random_string += str(random_str_seq[random.randint(0, len(random_str_seq) - 1)])
    return random_string

def random_date(start, end):
    delta = end - start
    int_delta = (delta.days * 24 * 60 * 60) + delta.seconds
    random_second = random.randrange(int_delta)
    res = start + datetime.timedelta(seconds=random_second)
    return res.isoformat()

def gen_list():
    d1 = datetime.datetime.strptime('1/1/2008 1:30 PM', '%m/%d/%Y %I:%M %p')
    d2 = datetime.datetime.now()
    return {
        "uuid": str(uuid.uuid4()),
        "name": generate_random_string(8,1),
        "name_a": generate_random_string(8,2),
        "name_b": generate_random_string(8,2),
        "changed": random_date(d1,d2),
        "created": random_date(d1,d2),
    }

def gen_list_del():
    d1 = datetime.datetime.strptime('1/1/2008 1:30 PM', '%m/%d/%Y %I:%M %p')
    d2 = datetime.datetime.now()
    return {
        "list": str(uuid.uuid4()),
        "time": random_date(d1,d2),
    }

def gen_entry_del(list):
    d1 = datetime.datetime.strptime('1/1/2008 1:30 PM', '%m/%d/%Y %I:%M %p')
    d2 = datetime.datetime.now()
    return {
        "list": str(list["uuid"]),
        "time": random_date(d1,d2),
        "entry": str(uuid.uuid4())
    }

def gen_entry_del_from_lists(lists):
    lst = random.randrange(0,len(lists))
    return gen_entry_del(lists[lst])


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

# register via JWT, app part
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
    if res.status_code != 200:
        print(res)
        print(f"response: {res.text}")
        print(res.headers)
        raise Exception(f"Register failed with {res.status_code}")

# login via JWT; app part
def login(server, privkey, user_id):
    curSession = requests.Session()
    payload = jwt_payload(server,user_id, "login")
    encoded = jwt.encode(payload,key= privkey, algorithm="ES256")

    payload = {"iss": str(user_id), "proof": encoded}
    res = curSession.post(url+"/api/v1/account/login/key",json=payload)
    print(res)
    print(f"response: {res.text}")
    print(res.headers)
    if res.status_code != 200:
        raise Exception(f"Login failed with {res.status_code}")
    return curSession

def account_info(session):
    res = session.get(url+"/api/v1/account/info")
    if res.status_code != 200:
        print(res)
        print(f"response: {res.text}")
        raise Exception(f"User info failed with {res.status_code}")
    return res.json()

# bind password to account via JWT; app part
def bind_password(session,email,password):
    data = {"email": email,"password": password}
    res = session.post(url+"/api/v1/account/register/password",json=data)
    if res.status_code != 200:
        print(res)
        print(f"response: {res.text}")
        raise Exception(f"Password bind failed with {res.status_code}")

# login via password web/app
def login_password(email,password):
    curSession = requests.Session()
    data = {'email': email,'password': password}
    res = curSession.post(url+"/api/v1/account/login/password",json=data)
    if res.status_code != 200:
        print(res)
        print(f"response: {res.text}")
        raise Exception(f"Password bind failed with {res.status_code}")
    return curSession

def list_sync_changed(session,client,lists):
    data = {'client': str(client), 'lists': lists}
    res = session.post(url+"/api/v1/sync/lists/changed",json=data)
    if res.status_code != 200:
        print(res)
        print(f"response: {res.text}")
        print(f"send data: {data}")
        print(f"send raw: {res.request.body}")
        raise Exception(f"List change sync failed with {res.status_code}")
    return res.json()

def list_sync_del(session,client,lists):
    data = {'client': str(client), 'lists': lists}
    res = session.post(url+"/api/v1/sync/lists/deleted",json=data)
    if res.status_code != 200:
        print(res)
        print(f"response: {res.text}")
        print(f"send data: {data}")
        print(f"send raw: {res.request.body}")
        raise Exception(f"List change sync failed with {res.status_code}")
    return res.json()

def entry_sync_del(session,client,entries):
    data = {'client': str(client), 'entries': entries}
    res = session.post(url+"/api/v1/sync/entries/deleted",json=data)
    if res.status_code != 200:
        print(res)
        print(f"response: {res.text}")
        print(f"send data: {data}")
        print(f"send raw: {res.request.body}")
        raise Exception(f"List change sync failed with {res.status_code}")
    return res.json()


server = server_info()
user_id = uuid.uuid4()

startTime = time()
print("registering")
register(server, pubkey, privkey, user_id)
print("logging in")
session = login(server, privkey, user_id)
print(account_info(session))

print("binding password")
email = str(uuid.uuid4())
password = str(uuid.uuid4())
bind_password(session,email,password)
print("logging in via password")
session = login_password(email,password)
print(account_info(session))
executionTime = (time() - startTime)
print('Execution time in seconds: ' + str(executionTime))
print("syncing lists")
client = uuid.uuid4()
for x in range(1):
    lists = [gen_list(),gen_list(),gen_list()]
    lists_del = [gen_list_del(),gen_list_del(),gen_list_del()]
    entries_del = [gen_entry_del_from_lists(lists),gen_entry_del_from_lists(lists),gen_entry_del_from_lists(lists)]
    #print(lists)
    startTime = time()
    list_sync_changed(session,client,lists)
    entry_sync_del(session,client,entries_del)
    list_sync_del(session,client,lists_del)
    executionTime = (time() - startTime)
    print('Execution time in seconds: ' + str(executionTime))

    #print(res)