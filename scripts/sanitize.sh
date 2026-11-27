#!/usr/bin/env bash
# Lays out every HTML mail in the body cache, and the .eml fixtures, under
# ASan and UBSan, and fails on any report. Mail is untrusted input to C++.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
out=$root/target/sanitize
lite=$root/vendor/litehtml
gumbo=$lite/src/gumbo/src
flags=(-fsanitize=address,undefined -fno-omit-frame-pointer -g -O1 -w)

rm -rf "$out"
mkdir -p "$out/obj" "$out/corpus"

cat >"$out/driver.cpp" <<'EOF'
#include "cell_grid.h"
#include <fstream>
#include <sstream>
int main(int argc, char** argv)
{
    for(int i = 1; i < argc; i++)
    {
        std::ifstream     f(argv[i]);
        std::stringstream s;
        s << f.rdbuf();
        std::string html = s.str();
        if(cg_grid* g = cg_render(html.data(), html.size(), 74)) cg_free(g);
    }
}
EOF

for c in "$gumbo"/*.c; do
    clang "${flags[@]}" -std=c99 -I"$gumbo" -c "$c" -o "$out/obj/g_$(basename "$c").o" &
done
for c in "$lite"/src/*.cpp; do
    clang++ "${flags[@]}" -std=c++17 -I"$lite/include" -I"$lite/include/litehtml" -I"$lite/src" -I"$gumbo" \
        -c "$c" -o "$out/obj/l_$(basename "$c").o" &
done
for c in "$root"/cpp/*.cpp "$out/driver.cpp"; do
    clang++ "${flags[@]}" -std=c++17 -I"$lite/include" -I"$root/cpp" -c "$c" -o "$out/obj/c_$(basename "$c").o" &
done
wait
clang++ "${flags[@]}" "$out"/obj/*.o -o "$out/drv"

python3 -I "$root/scripts/corpus.py" "${1:-}" "$out/corpus"

count=$(find "$out/corpus" -name '*.html' | wc -l | tr -d ' ')
export ASAN_OPTIONS=detect_leaks=0:halt_on_error=0 UBSAN_OPTIONS=print_stacktrace=1
find "$out/corpus" -name '*.html' -print0 | xargs -0 -n 20 -P 8 "$out/drv" 2>"$out/report.txt" || true

if grep -qE 'runtime error|ERROR: AddressSanitizer' "$out/report.txt"; then
    grep -E 'runtime error|ERROR: AddressSanitizer' "$out/report.txt" | sort | uniq -c | sort -rn
    echo "sanitizers reported errors on $count mails, full report in $out/report.txt" >&2
    exit 1
fi
echo "$count mails clean under ASan and UBSan"
