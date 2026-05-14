import socket

def send_resp(cmd):
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect(("127.0.0.1", 6379))
    
    resp = ""
    if isinstance(cmd, list):
        resp += f"*{len(cmd)}\r\n"
        for arg in cmd:
            resp += f"${len(arg)}\r\n{arg}\r\n"
    else:
        resp = cmd
        
    s.sendall(resp.encode())
    data = s.recv(1024)
    s.close()
    return data.decode()

print(f"PING: {send_resp(['PING'])}")
print(f"SET: {send_resp(['SET', 'mykey', 'Hello_RESP'])}")
print(f"GET: {send_resp(['GET', 'mykey'])}")
