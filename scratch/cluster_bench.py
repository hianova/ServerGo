import subprocess
import time
import os
import json

# Kill any existing processes
os.system("lsof -t -i:6379,6380,6381 | xargs kill -9 2>/dev/null")

# Start 3 nodes in release mode
ports = [6379, 6380, 6381]
processes = []

print("Starting Cluster of 3 nodes...")
for i, port in enumerate(ports):
    node_id = i + 1
    cmd = ["./target/release/ServerGo", "--port", str(port), "--id", str(node_id)]
    p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    processes.append(p)
    print(f"Node {node_id} starting on port {port}...")

# Wait for nodes to start
time.sleep(5)

# Run benchmark against each node
results = {}
for port in ports:
    print(f"Benchmarking Node on port {port}...")
    try:
        output = subprocess.check_output([
            "redis-benchmark", "-h", "127.0.0.1", "-p", str(port), 
            "-t", "set,get", "-q", "-c", "50", "-n", "100000"
        ], stderr=subprocess.STDOUT, timeout=60)
        results[port] = output.decode()
    except subprocess.TimeoutExpired:
        results[port] = "Error: Benchmark timed out"
    except Exception as e:
        results[port] = f"Error: {e}"

# Kill processes
for p in processes:
    p.terminate()

# Save results
with open("cluster_results.json", "w") as f:
    json.dump(results, f, indent=2)

print("Done. Results saved to cluster_results.json")
