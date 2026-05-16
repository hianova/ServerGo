#!/bin/bash
set -e

# Node 1 is the bootnode
if [ "$NODE_ID" = "1" ]; then
    echo "Starting as bootnode (Node 1)..."
    # Run and capture PeerId if possible, or just run
    # For now, we'll just run and expect others to connect if they have the ID
    # In a real scenario, we'd write the ticket to /shared/bootnode.ticket
    ./ServerGo --id "$NODE_ID" --port 6379 --bind 0.0.0.0 --budget "$BUDGET" --trust-mode "$TRUST_MODE" --control-mode "$CONTROL_MODE"
else
    echo "Starting as node $NODE_ID..."
    # Wait for bootnode (optional, but good for stability)
    # sleep 5 
    ./ServerGo --id "$NODE_ID" --port 6379 --bind 0.0.0.0 --budget "$BUDGET" --trust-mode "$TRUST_MODE" --control-mode "$CONTROL_MODE"
fi
