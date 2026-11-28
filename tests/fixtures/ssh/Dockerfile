ARG BASE=scratch
FROM ${BASE}
ARG SERVER
RUN case "$SERVER" in \
    ubuntu|debian) apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends openssh-server && rm -rf /var/lib/apt/lists/* ;; \
    fedora|almalinux) dnf install -y openssh-server shadow-utils && dnf clean all ;; \
    arch) pacman -Syu --needed --noconfirm openssh && pacman -Scc --noconfirm ;; \
    alpine) apk add --no-cache openssh-server shadow ;; \
    opensuse) zypper --non-interactive install --no-recommends openssh-server shadow && zypper clean --all ;; \
    asyncssh) pip install --no-cache-dir asyncssh==2.24.2 ;; \
    *) exit 1 ;; \
    esac
COPY openssh.sh asyncssh_server.py /usr/local/lib/tfm-ssh/
