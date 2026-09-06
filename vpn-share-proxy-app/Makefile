.PHONY: all build-docker clean help

all: build-docker

build-docker:
	@chmod +x docker/build.sh
	@./docker/build.sh

clean:
	@rm -rf dist app/build

help:
	@echo "VPN Share Proxy Android App (vpn-share-proxy-app) Build System"
	@echo ""
	@echo "Targets:"
	@echo "  make build-docker   - Build APK completely inside Docker container"
	@echo "  make clean          - Clean output build artifacts"
	@echo ""
