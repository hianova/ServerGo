.PHONY: build up down chaos oom-test dashboard clean

build:
	docker compose build

up:
	docker compose up -d

down:
	docker compose down

chaos:
	python3 tools/chaos_test.py

oom-test:
	python3 tools/ram_crusher.py

dashboard:
	python3 tools/dashboard.py

clean:
	docker compose down -v
	rm -rf data/
	rm -f *.wal
	rm -f leader_wal.log
