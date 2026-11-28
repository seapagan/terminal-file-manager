#!/bin/sh
set -eu

mkdir -p /run/sshd
useradd --create-home --shell /bin/sh tfm
printf 'tfm:%s\n' "$(cat /fixture/password)" | chpasswd
sh /fixture/layout.sh /home/tfm
mkdir -m 700 /home/tfm/.ssh
cp /fixture/authorized_keys /home/tfm/.ssh/authorized_keys
chmod 600 /home/tfm/.ssh/authorized_keys
chown -R tfm /home/tfm
cp /fixture/host /tmp/tfm-host
chmod 600 /tmp/tfm-host
sshd -V
# Use the distribution configuration and its crypto policy.
exec /usr/sbin/sshd -D -e -p 2022 -h /tmp/tfm-host \
    -o 'PasswordAuthentication yes' \
    -o 'AllowUsers tfm' -o 'DisableForwarding yes' -o 'ForceCommand internal-sftp -R' \
    -o 'Subsystem sftp internal-sftp'
