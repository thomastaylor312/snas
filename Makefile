.PHONY: build-linux build-linux-bins build-linux-pam e2e pam-e2e pam-e2e-down

build-linux: build-linux-bins build-linux-pam

build-linux-bins:
	cargo zigbuild --locked --release --target x86_64-unknown-linux-gnu --package snas --bins --target-dir target/zigbuild

build-linux-pam:
	rm -rf target/pam-build
	mkdir -p target/release
	docker build --platform=linux/amd64 -f tests/pam-e2e/pam-build.Dockerfile --target artifact --output type=local,dest=target/pam-build .
	cp target/pam-build/libsnas_pam_socket.so target/release/libsnas_pam_socket.so

e2e: build-linux
	bash -lc 'set -u; set -o pipefail; docker compose -f tests/pam-e2e/docker-compose.yml up --build --abort-on-container-exit --exit-code-from pamtest; status=$$?; if [ $$status -ne 0 ]; then docker compose -f tests/pam-e2e/docker-compose.yml logs --no-color || true; fi; docker compose -f tests/pam-e2e/docker-compose.yml down -v; exit $$status'

pam-e2e: e2e

pam-e2e-down:
	docker compose -f tests/pam-e2e/docker-compose.yml down -v || true
