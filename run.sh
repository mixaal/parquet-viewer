#!/bin/bash
#
D=$(dirname $0)
VW="$D/target/release/parquet-viewer" 
[ -x "$VW" ] && {
	chmod +x "$VW"
	exec $VW
}

$D/build.sh
exec $VW
