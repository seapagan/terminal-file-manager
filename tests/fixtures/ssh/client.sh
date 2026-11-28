#!/bin/bash
set -euo pipefail
umask 077

root=/tmp/tfm-ssh
seed=/fixture
export HOME="$root/local" SSH_AUTH_SOCK="$root/agent"
unset SSH_AGENT_PID SSH_ASKPASS SSH_ASKPASS_REQUIRE

prepare() {
    mkdir -p "$HOME" "$root"
    ln -s "$seed" "$root/fixture"
    touch "$HOME/local-marker"
    chmod 755 "$seed"
    cp /opt/tfm-ssh/{layout,sftpgo}.sh "$seed"
    od -An -N16 -tx1 /dev/urandom | tr -d ' \n' > "$root/passphrase"
    od -An -N16 -tx1 /dev/urandom | tr -d ' \n' > "$seed/password"
    ssh-keygen -q -t ed25519 -C '' -N '' -f "$seed/host"
    ssh-keygen -q -t rsa -b 3072 -C '' -N '' -f "$root/client"
    ssh-keygen -q -t ed25519 -C '' -N "$(cat "$root/passphrase")" -f "$root/encrypted"
    ssh-keygen -q -t ed25519 -C '' -N '' -f "$root/wrong"
    cat "$root/client.pub" "$root/encrypted.pub" > "$seed/authorized_keys"
    printf '{"version":16,"users":[{"id":1,"status":1,"username":"tfm","password":"%s","home_dir":"/tmp/tfm-home","public_keys":["%s","%s"],"permissions":{"/":["list","download"]}}]}\n' \
        "$(cat "$seed/password")" "$(cat "$root/client.pub")" "$(cat "$root/encrypted.pub")" > "$seed/users.json"
    chmod 644 "$seed"/{password,host,authorized_keys,users.json,layout.sh,sftpgo.sh}
    ssh-agent -a "$SSH_AUTH_SOCK" >/dev/null
    ssh-add "$root/client" </dev/null
}

ssh_ready() (
    local banner
    exec 3<> /dev/tcp/server/2022
    IFS= read -r -t 1 banner <&3 && [[ "$banner" == SSH-2.0-* ]]
)

run_contract() {
    local deadline=$((SECONDS + 15)) missing=0
    if [[ "$1" == asyncssh ]]; then missing=1; fi
    while (( SECONDS < deadline )); do
        if ssh_ready 2>/dev/null; then
            TFM_SSH_INTEROP_HOST=server TFM_SSH_INTEROP_PORT=2022 TFM_SSH_INTEROP_ROOT="$root" \
                TFM_SSH_INTEROP_PASSPHRASE="$(cat "$root/passphrase")" TFM_SSH_INTEROP_MISSING_METADATA="$missing" \
                TFM_SSH_INTEROP_EXAMPLE=/opt/tfm-ssh/ssh_connect \
                timeout -k 5s 90s /opt/tfm-ssh/contract interoperability:: --ignored --test-threads=1
            return
        fi
        sleep 0.1
    done
    echo "SSH fixture did not become ready" >&2
    return 1
}

case "$1" in
    prepare) prepare ;;
    run) run_contract "$2" ;;
    redact)
        TFM_FIXTURE_PASSWORD=$(cat "$seed/password") TFM_FIXTURE_PASSPHRASE=$(cat "$root/passphrase") \
            awk '
                /-----BEGIN .*PRIVATE KEY-----/ { key = 1 }
                !key {
                    gsub(ENVIRON["TFM_FIXTURE_PASSWORD"], "[redacted]")
                    gsub(ENVIRON["TFM_FIXTURE_PASSPHRASE"], "[redacted]")
                    print
                }
                /-----END .*PRIVATE KEY-----/ { key = 0 }
            '
        ;;
esac
