#!/bin/sh
set -eu

cd "$(dirname "$0")/.githooks"

for hook in *
do
    ln -fsn "../../.githooks/$hook" "../.git/hooks/$hook"
done