import socket
import time
import threading

CONCURRENCY = 10
REQUESTS_PER_THREAD = 1000

def bench_thread(thread_id, results):
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect(("127.0.0.1", 6379))
    s.settimeout(2.0)
    
    start = time.time()
    for i in range(REQUESTS_PER_THREAD):
        key = f"key_{thread_id}_{i}"
        val = f"val_{i}"
        
        # SET
        cmd_set = f"*3\r\n$3\r\nSET\r\n${len(key)}\r\n{key}\r\n${len(val)}\r\n{val}\r\n"
        s.sendall(cmd_set.encode())
        s.recv(1024)
        
        # GET
        cmd_get = f"*2\r\n$3\r\nGET\r\n${len(key)}\r\n{key}\r\n"
        s.sendall(cmd_get.encode())
        s.recv(1024)
        
    duration = time.time() - start
    results.append(duration)
    s.close()

results = []
threads = []
for i in range(CONCURRENCY):
    t = threading.Thread(target=bench_thread, args=(i, results))
    threads.append(t)
    t.start()

for t in threads:
    t.join()

total_requests = CONCURRENCY * REQUESTS_PER_THREAD * 2 # SET + GET
total_duration = max(results)
tps = total_requests / total_duration

print(f"Total Requests: {total_requests}")
print(f"Total Duration: {total_duration:.2f}s")
print(f"Throughput: {tps:.2f} req/s")
print(f"Avg Latency: {(total_duration / total_requests * 1000 * CONCURRENCY):.2f}ms")
