import redis
import time
import subprocess
import os

# Configuration
REDIS_HOST = 'localhost'
REDIS_PORT = 6389 # From docker-compose
SERVERGO_HOST = 'localhost'
SERVERGO_PORT = 6379 # Node 1

DATA_SIZE_GB = 6
CHUNK_SIZE = 1000
VAL_SIZE = 1024 # 1KB per value

def get_docker_mem(container_name):
    cmd = f"docker stats {container_name} --no-stream --format '{{{{.MemUsage}}}}'"
    res = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    return res.stdout.strip()

def fill_data(host, port, name):
    print(f"--- Filling {name} with {DATA_SIZE_GB}GB of data ---")
    r = redis.Redis(host=host, port=port)
    start_time = time.time()
    
    # We'll fill in batches to see it grow
    total_keys = (DATA_SIZE_GB * 1024 * 1024 * 1024) // VAL_SIZE
    keys_per_step = total_keys // 10
    
    for i in range(10):
        pipe = r.pipeline()
        for j in range(keys_per_step // CHUNK_SIZE):
            for k in range(CHUNK_SIZE):
                key = f"key_{i}_{j}_{k}"
                pipe.set(key, b"x" * VAL_SIZE)
            try:
                pipe.execute()
            except Exception as e:
                print(f"Error writing to {name}: {e}")
                break
        
        mem = get_docker_mem(name)
        print(f"Step {i+1}/10: Memory Usage of {name}: {mem}")
    
    end_time = time.time()
    print(f"Finished filling {name} in {end_time - start_time:.2f}s")

def main():
    print("Starting RAM Crusher Test...")
    
    # 1. Test Redis
    try:
        fill_data(REDIS_HOST, REDIS_PORT, "redis-comparison")
    except Exception as e:
        print(f"Redis Test failed/halted: {e}")

    print("\n" + "="*40 + "\n")

    # 2. Test ServerGo
    try:
        fill_data(SERVERGO_HOST, SERVERGO_PORT, "server-go-node1")
    except Exception as e:
        print(f"ServerGo Test failed: {e}")

if __name__ == "__main__":
    main()
