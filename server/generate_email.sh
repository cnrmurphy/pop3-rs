#!/bin/bash

set -e

USERNAME="$1"
MAILDIR="./Maildir/$USERNAME/new"

if [ -z "$USERNAME" ]; then
  echo "Usage: $0 <username>"
  exit 1
fi

mkdir -p "$MAILDIR"

TIMESTAMP=$(date +%s)
MICRO=$(date +%N | cut -c1-6)         # microseconds
PID=$$
HOSTNAME=$(hostname)
FILENAME="${TIMESTAMP}.M${MICRO}P${PID}.${HOSTNAME}"

FULLPATH="$MAILDIR/$FILENAME"

cat > "$FULLPATH" <<EOF
From: mock@example.com
To: $USERNAME@example.com
Subject: Mock Email
Date: $(date -R)
Message-ID: <${TIMESTAMP}.${PID}@${HOSTNAME}>

Hello $USERNAME,

This is a test message generated at $(date).

Regards,
Mock Mailer
EOF

echo "Mock email written to: $FULLPATH"
