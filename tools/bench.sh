#!/bin/bash

# ServerGo Realistic Benchmark Script
# Requirements: redis-benchmark must be installed.

PORT=${1:-6379}
HOST=${2:-127.0.0.1}
REQUESTS=${3:-100000}
CLIENTS=${4:-100}
KEY_RANGE=${5:-1000000}

echo "--- Starting ServerGo Benchmark on $HOST:$PORT ---"
echo "Requests: $REQUESTS, Clients: $CLIENTS, Key Range: $KEY_RANGE"

# Run SET benchmark with random keys
echo "Running SET benchmark..."
redis-benchmark -h $HOST -p $PORT -n $REQUESTS -c $CLIENTS -t set -r $KEY_RANGE --precision 3 --csv

# Run GET benchmark with random keys
echo "Running GET benchmark..."
redis-benchmark -h $HOST -p $PORT -n $REQUESTS -c $CLIENTS -t get -r $KEY_RANGE --precision 3 --csv

# Run MSET benchmark (if supported)
echo "Running MSET benchmark..."
redis-benchmark -h $HOST -p $PORT -n $REQUESTS -c $CLIENTS -t mset -r $KEY_RANGE --csv

echo "--- Benchmark Complete ---"
