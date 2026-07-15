#!/bin/sh
set -eu

usage() {
	printf '%s\n' \
		'usage: scripts/prooflane-shadow-openbrief.sh run <contract>' \
		'       scripts/prooflane-shadow-openbrief.sh delivery <prooflane-run-id>' >&2
}

if [ "$#" -ne 2 ]; then
	usage
	exit 2
fi

operation=$1
value=$2
openbrief_binary=${OPENBRIEF_BINARY:-openbrief}
script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
sibling_prooflane=$script_dir/../../prooflane/bin/prooflane

if [ -n "${PROOFLANE_BINARY:-}" ]; then
	prooflane_binary=$PROOFLANE_BINARY
elif [ -x "$sibling_prooflane" ]; then
	prooflane_binary=$sibling_prooflane
else
	prooflane_binary=prooflane
fi

case $operation in
	run)
		exec "$prooflane_binary" dogfood openbrief run \
			--contract "$value" \
			--config-binary "$openbrief_binary" \
			--brief-binary "$openbrief_binary"
		;;
	delivery)
		exec "$prooflane_binary" dogfood openbrief delivery \
			--run "$value" \
			--brief-binary "$openbrief_binary"
		;;
	*)
		usage
		exit 2
		;;
esac
