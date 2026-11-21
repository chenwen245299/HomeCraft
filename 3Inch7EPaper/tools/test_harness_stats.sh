#!/bin/sh
# Compatibility entry point for the old test command; source aggregation was removed.
set -eu
exec sh "$(dirname "$0")/test_display_snapshots.sh"
