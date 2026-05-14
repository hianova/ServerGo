import subprocess
import time
import sys

DURATION_SECONDS = 3600 # 1 hour
PORT = 6379
CLIENTS = 50
CONCURRENCY = 100

def run_aging_test():
    print(f"--- Starting 1-Hour Aging Test on Port {PORT} ---")
    start_time = time.time()
    end_time = start_time + DURATION_SECONDS
    
    count = 0
    while time.time() < end_time:
        remaining = int(end_time - time.time())
        print(f"[{count}] Running load... {remaining}s remaining", end='\r')
        try:
            # Run a quick burst of 10k requests
            subprocess.check_output([
                "redis-benchmark", "-h", "127.0.0.1", "-p", str(PORT), 
                "-t", "set,get", "-n", "10000", "-c", str(CLIENTS), "-q", "--precision", "3"
            ], timeout=60)
            count += 1
        except subprocess.TimeoutExpired:
            print("\nError: Benchmark timed out!")
        except Exception as e:
            print(f"\nError: {e}")
            break
            
        # Small sleep between bursts to allow background tasks to catch up
        time.sleep(1)
        
    print(f"\n--- Aging Test Complete. Total bursts: {count} ---")

if __name__ == "__main__":
    run_aging_test()
