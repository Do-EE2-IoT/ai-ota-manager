#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <file>" >&2
  exit 2
fi

sha256sum "$1" | awk '{print $1}'
