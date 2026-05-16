import redis
import time
import os

nodes = [
    ("Node 1", "localhost", 6379),
    ("Node 2", "localhost", 6380),
    ("Node 3", "localhost", 6381),
    ("Node 4", "localhost", 6382),
    ("Node 5", "localhost", 6383),
]

def get_node_info(host, port):
    try:
        r = redis.Redis(host=host, port=port, socket_timeout=0.5)
        info = r.execute_command("INFO")
        if isinstance(info, bytes):
            info = info.decode('utf-8')
        return info
    except Exception:
        return "OFFLINE"

def main():
    while True:
        os.system('clear')
        print("="*60)
        print(" ServerGo Cluster Monitoring Dashboard ")
        print("="*60)
        print(f"{'Node':<10} | {'Status':<10} | {'Details'}")
        print("-" * 60)
        
        for name, host, port in nodes:
            status = get_node_info(host, port)
            if status == "OFFLINE":
                print(f"{name:<10} | \033[91mOFFLINE\033[0m    | -")
            else:
                # Basic parsing of our custom INFO string
                # format: node_id:X\r\nversion:Y\r\ntrust_mode:Z\r\ncontrol_mode:W\r\n
                details = status.replace('\r', '').replace('\n', ' | ')
                print(f"{name:<10} | \033[92mONLINE\033[0m     | {details}")
        
        print("-" * 60)
        print("Press Ctrl+C to exit. Updates every 2 seconds.")
        time.sleep(2)

if __name__ == "__main__":
    main()
