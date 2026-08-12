#!/usr/bin/env bash
#
# Split a single cbindgen-generated header into the public, content-named
# headers the crate ships, and write the umbrella header that includes them.
#
#   generate_c_abi_public_header.sh <cbindgen-output> <umbrella-header-path>
#
# The split point is the declaration named by C_ABI_SPLIT_BOUNDARY: everything
# up to and including it (the type definitions and the session-core entry
# points) lands in the first header, the remaining declarations in the second.
# The Python module initialiser is not part of the C ABI and is dropped.

set -euo pipefail

if [ "$#" -ne 2 ]; then
	echo "usage: $0 <cbindgen-output> <umbrella-header-path>" >&2
	exit 2
fi

src=$1
dest=$2
boundary=${C_ABI_SPLIT_BOUNDARY:-machbus_ecu_identification_free}

if [ ! -f "$src" ]; then
	echo "$0: generated header '$src' not found" >&2
	exit 1
fi

name=$(basename "$dest" .h)
outdir=$(dirname "$dest")
splitdir="$outdir/$name"
guard=$(printf '%s' "$name" | tr '[:lower:]-' '[:upper:]_')_H

types_header="$splitdir/c_abi_types_and_session_core.h"
funcs_header="$splitdir/c_abi_protocol_functions_and_builders.h"

mkdir -p "$splitdir"

# First line of the boundary declaration, then the line that terminates it.
decl_start=$(awk -v b="$boundary" '/^[A-Za-z_]/ && index($0, b "(") { print NR; exit }' "$src")
if [ -z "$decl_start" ]; then
	echo "$0: split boundary '$boundary' not found in $src" >&2
	exit 1
fi
boundary_end=$(awk -v s="$decl_start" 'NR >= s && /;[[:space:]]*$/ { print NR; exit }' "$src")

# The extern "C" block closes on the last #endif before the include guard's.
extern_close=$(grep -n '^#endif  // __cplusplus' "$src" | tail -1 | cut -d: -f1)
if [ -z "$extern_close" ]; then
	echo "$0: could not locate the closing extern \"C\" guard in $src" >&2
	exit 1
fi

# Types and session-core entry points: skip the outer include guard.
sed -n "3,${boundary_end}p" "$src" >"$types_header"

# Remaining declarations, minus the Python initialiser and its doc comment.
sed -n "$((boundary_end + 2)),${extern_close}p" "$src" |
	awk '
		/^\/\*\*/ { buffered = $0 "\n"; holding = 1; next }
		holding   { buffered = buffered $0 "\n"
		            if ($0 !~ /\*\//) next
		            holding = 0; next }
		/PyInit_machbus|PyObject/ { buffered = ""; next }
		{ printf "%s", buffered; buffered = ""; print }
		END { printf "%s", buffered }
	' |
	cat -s >"$funcs_header"
printf '\n' >>"$funcs_header"

cat >"$dest" <<EOF
#ifndef ${guard}
#define ${guard}

/* Content-named generated C ABI headers; run make bind-c to regenerate. */
#include "${name}/$(basename "$types_header")"
#include "${name}/$(basename "$funcs_header")"

#endif  /* ${guard} */
EOF
