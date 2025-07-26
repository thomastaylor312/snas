FROM ubuntu:24.04 AS runtime
ARG BIN_TARGET_DIR=target/zigbuild/x86_64-unknown-linux-gnu/release
ARG PAM_TARGET_DIR=target/release
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libgcc-s1 \
    libstdc++6 \
    libpam0g \
    libpam-modules \
    libpam-modules-bin \
    passwd \
    build-essential \
    libpam0g-dev \
    procps \
    tini \
    && rm -rf /var/lib/apt/lists/*

COPY ${BIN_TARGET_DIR}/snas-server /usr/local/bin/snas-server
COPY ${BIN_TARGET_DIR}/snas /usr/local/bin/snas
COPY ${PAM_TARGET_DIR}/libsnas_pam_socket.so /lib/security/pam_snas_socket.so
RUN arch_dir="$(gcc -print-multiarch)" && \
    if [ -n "${arch_dir}" ] && [ -d "/lib/${arch_dir}" ]; then \
      mkdir -p "/lib/${arch_dir}/security" && \
      cp /lib/security/pam_snas_socket.so "/lib/${arch_dir}/security/pam_snas_socket.so"; \
    fi

COPY tests/pam-e2e/pam.d/snas /etc/pam.d/snas

COPY tests/pam-e2e/pam_test.c /usr/local/src/pam_test.c
RUN cc -O2 -Wall -o /usr/local/bin/pam_test /usr/local/src/pam_test.c -lpam

COPY tests/pam-e2e/run.sh /usr/local/bin/run.sh
RUN chmod +x /usr/local/bin/run.sh

COPY tests/pam-e2e/setup_probe.sh /usr/local/bin/setup_probe.sh
RUN chmod +x /usr/local/bin/setup_probe.sh

ENV SNAS_PAM_SOCKET_PATH=/run/snas.sock
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/run.sh"]
