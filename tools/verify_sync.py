import redis
import time
import subprocess
import os

def test_distributed_sync():
    print("--- Starting Distributed Sync Test ---")
    
    # 1. Start Node 1 (Port 6379)
    print("Starting Node 1...")
    node1 = subprocess.Popen(["cargo", "run", "--", "--id", "1", "--port", "6379", "--trust-mode", "full"], 
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    time.sleep(5) # Wait for node to start and iroh to bind
    
    # Get Node 1's Iroh Peer ID (Mock or real)
    # For this test, we assume local discovery or manual peer adding if needed.
    
    # 2. Start Node 2 (Port 6380) and connect to Node 1
    # We need Node 1's iroh node id to connect. 
    # In this mock setup, we can just start them and they should see each other if on the same machine/network
    # or we can extract the PeerId from logs.
    
    print("Starting Node 2...")
    node2 = subprocess.Popen(["cargo", "run", "--", "--id", "2", "--port", "6380", "--trust-mode", "full"],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    time.sleep(5)

    # 3. Write to Node 1
    print("Writing 'hello' -> 'world' to Node 1 (6379)...")
    r1 = redis.Redis(host='127.0.0.1', port=6379)
    r1.set('hello', 'world')
    
    time.sleep(2) # Wait for P2P propagation
    
    # 4. Read from Node 2
    print("Reading 'hello' from Node 2 (6380)...")
    r2 = redis.Redis(host='127.0.0.1', port=6380)
    val = r2.get('hello')
    
    if val and val.decode('utf-8') == 'world':
        print("✅ SUCCESS: Data propagated from Node 1 to Node 2!")
    else:
        print(f"❌ FAILURE: Data mismatch or not found on Node 2. Got: {val}")

    # Cleanup
    node1.terminate()
    node2.terminate()

if __name__ == "__main__":
    test_distributed_sync()
