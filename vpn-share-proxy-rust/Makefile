.PHONY: all build-docker clean run-ctl help

all: build-docker

build-docker:
	@chmod +x docker/build.sh
	@./docker/build.sh

clean:
	@rm -rf target/dist

help:
	@echo "VPN Share Proxy Rust (vpn-share-proxy) Build System"
	@echo ""
	@echo "Targets:"
	@echo "  make build-docker   - Cross-compile static aarch64 binary inside Docker"
	@echo "  make clean          - Clean output build artifacts"
	@echo ""
