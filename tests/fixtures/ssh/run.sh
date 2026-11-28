#!/bin/sh
set -eu

fixtures=$(CDPATH='' cd "$(dirname "$0")" && pwd)
cd "$fixtures/../../.."
if [ "$#" -gt 1 ]; then
    echo "Usage: $0 [smoke|full|SERVER]" >&2
    exit 2
fi
selection=${1:-smoke}
rows=$(awk -F, -v selection="$selection" 'NR > 1 && (selection == $1 || selection == "full" ||
    (selection == "smoke" && $1 ~ /^(debian|asyncssh|sftpgo)$/)) { print }' "$fixtures/images.csv")
if [ -z "$rows" ]; then
    echo "Unknown SSH fixture: $selection (see images.csv)" >&2
    exit 2
fi

if ! command -v docker >/dev/null || ! docker buildx version >/dev/null; then
    echo "The SSH matrix requires Docker with BuildKit (buildx)." >&2
    exit 2
fi
if [ "$(docker info --format '{{.OSType}}')" != linux ]; then
    echo "Start Docker with a Linux container engine before running the SSH matrix." >&2
    exit 2
fi

# The host keeps only Docker image IDs. Credentials stay in the containers and fixture volume.
state=$(mktemp -d "${TMPDIR:-/tmp}/tfm-ssh-interop.XXXXXX")
prefix=tfm-ssh-interop-${state##*.}
client=$prefix-client
server=$prefix-server
network=
volume=
remove_container() {
    id=$(docker container ls -aq --filter "name=^/$1$") || return 1
    if [ -n "$id" ]; then docker rm -fv "$id" >/dev/null; fi
}
finish() {
    status=$?
    trap - 0
    remove_container "$server" || status=1
    remove_container "$client" || status=1
    if [ -n "$network" ]; then docker network rm "$network" >/dev/null || status=1; fi
    if [ -n "$volume" ]; then docker volume rm "$volume" >/dev/null || status=1; fi
    rm -rf "$state"
    exit "$status"
}
trap finish 0
trap 'exit 130' INT
trap 'exit 143' TERM

docker build --load --pull --iidfile "$state/client" -f "$fixtures/Dockerfile.client" .
client_image=$(cat "$state/client")
network=$(docker network create --internal "$prefix")
volume=$(docker volume create "$prefix")
docker run -d --init --name "$client" --network "$network" \
    --mount "type=volume,src=$volume,dst=/fixture" "$client_image" sleep infinity >/dev/null
docker exec "$client" rustc --version
docker exec "$client" timeout -k 5s 60s bash /opt/tfm-ssh/client.sh prepare

start_server() {
    image=$2
    case "$1" in
        sftpgo) set -- /bin/sh /fixture/sftpgo.sh ;;
        asyncssh) set -- /bin/sh -c 'sh /fixture/layout.sh /home/tfm && exec python /usr/local/lib/tfm-ssh/asyncssh_server.py' ;;
        *) set -- /bin/sh /usr/local/lib/tfm-ssh/openssh.sh ;;
    esac
    if [ "$name" != sftpgo ]; then
        docker build --load --pull ${platform:+--platform="$platform"} --iidfile "$state/server" \
            --build-arg "BASE=$image" --build-arg "SERVER=$name" "$fixtures" || return 1
        image=$(cat "$state/server") || return 1
        rm "$state/server" || return 1
    fi
    entrypoint=$1
    shift
    docker run -d --init --name "$server" ${platform:+--platform="$platform"} \
        --network "$network" --network-alias server \
        --mount "type=volume,src=$volume,dst=/fixture,readonly" \
        --entrypoint "$entrypoint" "$image" "$@" >/dev/null || return 1
    docker inspect --format 'Container image {{.Image}}' "$server"
}

failed=
while IFS=, read -r name base platform; do
    echo "=== $name ==="
    echo "Base image $base"
    started=$(date +%s)
    if start_server "$name" "$base" && docker exec "$client" bash /opt/tfm-ssh/client.sh run "$name"; then
        echo "PASS $name ($(($(date +%s) - started))s)"
    else
        failed="$failed $name"
        echo "FAIL $name ($(($(date +%s) - started))s)" >&2
        docker logs --tail 40 "$server" 2>&1 | docker exec -i "$client" bash /opt/tfm-ssh/client.sh redact >&2 || true
    fi
    docker logs "$server" 2>&1 | awk '/^(OpenSSH_|SFTPGo |INFO:__main__:AsyncSSH )/ { sub(/^INFO:__main__:/, ""); print }'
    remove_container "$server"
done <<EOF_ROWS
$rows
EOF_ROWS
if [ -n "$failed" ]; then
    echo "Failed fixtures:$failed" >&2
    exit 1
fi
