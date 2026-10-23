#!/bin/bash
# Transactions in private prefixes with host libraries: not OS certification.
set -euo pipefail
if [ "${RETOOLKIT_FAKEROOT:-}" != 1 ]; then
    exec fakeroot env RETOOLKIT_FAKEROOT=1 bash "$0"
fi
cd "$(dirname "$0")/.."
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')
for edition in light full; do
    name="retoolkit-linux-$edition"
    stage=$(mktemp -d "$PWD/.build/check-$edition.XXXXXX")
    mkdir -p "$stage/deb/var/lib/dpkg" "$stage/rpm"
    touch "$stage/deb/var/lib/dpkg/status"
    deb="$PWD/dist/$edition/${name}_${version}_amd64.deb"
    rpmfile="$PWD/dist/$edition/${name}-${version}.x86_64.rpm"
    dpkg --log="$stage/deb/dpkg.log" --force-not-root --force-depends --root="$stage/deb" --install "$deb"
    "$stage/deb/usr/bin/retoolkit-linux" --version
    "$stage/deb/usr/bin/retoolkit-linux" --edition > "$stage/deb-edition.json"
    python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["edition"]==sys.argv[2]' "$stage/deb-edition.json" "$edition"
    desktop-file-validate "$stage/deb/usr/share/applications/io.github.retoolkitlinux.REtoolkit.desktop"
    if [ "$edition" = full ]; then
        "$stage/deb/usr/bin/retoolkit-linux" --diagnose > "$stage/full-detection.json"
        python3 -c 'import json,sys; x=json.load(open(sys.argv[1])); assert x["system"]["dependencies"]["java"]; assert all(any(t["id"]==k and t["path"] for t in x["tools"]) for k in ["ghidra","cutter","radare2"])' "$stage/full-detection.json"
        "$stage/deb/usr/lib/retoolkit-linux/full/bin/radare2" -v
    else
        test ! -e "$stage/deb/usr/lib/retoolkit-linux/full"
    fi
    if [ "${RETOOLKIT_CHECK_GUI:-0}" = 1 ]; then
        env -u LD_PRELOAD -u FAKEROOTKEY xvfb-run -a dbus-run-session -- env GDK_BACKEND=x11 GSK_RENDERER=cairo GTK_A11Y=none XDG_CONFIG_HOME="$stage/gui-config" XDG_DATA_HOME="$stage/gui-data" XDG_CACHE_HOME="$stage/gui-cache" "$stage/deb/usr/bin/retoolkit-linux" --gui-smoke-test
    fi
    dpkg --log="$stage/deb/dpkg.log" --force-not-root --root="$stage/deb" --remove "$name"
    test ! -e "$stage/deb/usr/bin/retoolkit-linux"
    test ! -e "$stage/deb/usr/share/retoolkit-linux/edition.json"
    rpm --dbpath "$stage/rpm/rpmdb" --initdb
    rpm --dbpath "$stage/rpm/rpmdb" --badreloc --relocate "/usr=$stage/rpm/usr" --nodeps --ignoreos --noscripts --notriggers -i "$rpmfile"
    "$stage/rpm/usr/bin/retoolkit-linux" --version
    "$stage/rpm/usr/bin/retoolkit-linux" --edition > "$stage/rpm-edition.json"
    python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["edition"]==sys.argv[2]' "$stage/rpm-edition.json" "$edition"
    desktop-file-validate "$stage/rpm/usr/share/applications/io.github.retoolkitlinux.REtoolkit.desktop"
    if [ "$edition" = full ]; then "$stage/rpm/usr/lib/retoolkit-linux/full/bin/radare2" -v; else test ! -e "$stage/rpm/usr/lib/retoolkit-linux/full"; fi
    rpm --dbpath "$stage/rpm/rpmdb" --nodeps --noscripts --notriggers -e "$name"
    test ! -e "$stage/rpm/usr/bin/retoolkit-linux"
    test ! -e "$stage/rpm/usr/share/retoolkit-linux/edition.json"
    echo "PASS $edition DEB/RPM metadata, private installation, executable detection and removal"
done
echo 'PASS private-prefix package transactions; dependencies bypassed for this test, host runtime reused. Not a clean Ubuntu/Fedora system test.'
