import subprocess
import time
import random

nodes = [
    "server-go-node1",
    "server-go-node2",
    "server-go-node3",
    "server-go-node4",
    "server-go-node5"
]

def run_cmd(cmd):
    print(f"Executing: {cmd}")
    return subprocess.run(cmd, shell=True, capture_output=True, text=True)

def apply_chaos(node):
    print(f"--- Injecting chaos into {node} ---")
    # Add 500ms latency and 30% packet loss
    run_cmd(f"docker exec {node} tc qdisc add dev eth0 root netem delay 500ms loss 30%")

def remove_chaos(node):
    print(f"--- Removing chaos from {node} ---")
    run_cmd(f"docker exec {node} tc qdisc del dev eth0 root netem")

def main():
    try:
        while True:
            # Pick 2 random nodes to fail
            failed_nodes = random.sample(nodes, 2)
            for node in failed_nodes:
                apply_chaos(node)
            
            print("Chaos injected. Waiting 15 seconds...")
            time.sleep(15)
            
            for node in failed_nodes:
                remove_chaos(node)
            
            print("Chaos removed. Waiting 10 seconds for recovery...")
            time.sleep(10)
    except KeyboardInterrupt:
        print("\nCleaning up...")
        for node in nodes:
            remove_chaos(node)

if __name__ == "__main__":
    main()
