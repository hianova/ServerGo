.PHONY: build up down chaos oom-test dashboard release release-linux clean

build:
	docker compose build

up:
	docker compose up -d

down:
	docker compose down

chaos:
	python3 ../tools/chaos_test.py

oom-test:
	python3 ../tools/ram_crusher.py

dashboard:
	python3 ../tools/dashboard.py

release:
	cargo build --release --manifest-path ../Cargo.toml
	mkdir -p ../install_packages/bin/macos
	cp ../target/release/ServerGo ../install_packages/bin/macos/

release-linux:
	docker build -f Dockerfile -t server-go-builder ..
	docker create --name temp-container server-go-builder
	mkdir -p ../install_packages/bin/linux_x64
	docker cp temp-container:/usr/local/bin/ServerGo ../install_packages/bin/linux_x64/ServerGo
	docker rm temp-container

clean:
	docker compose down -v
	rm -rf ../data/
	rm -f ../*.wal
	rm -f ../leader_wal.log
