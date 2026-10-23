# Release editions

`profiles.json` defines the Light/Full split. `scripts/package-editions.py` generates the DEB control files and RPM specs from the same profiles, with alternative-package relationships and shared XDG configuration.

- Light ships no third-party tools.
- Full ships the verified private payload Ghidra/Cutter/radare2/JDK.
- Native Full packages also declare distribution-managed baseline tools as dependencies.
- Full AppImage ships the private payload only; it never installs APT/DNF packages on the host.

Native install locations:

- `/usr/bin/retoolkit-linux`: native ELF.
- `/usr/share/retoolkit-linux/edition.json`: edition identity and content declaration.
- `/usr/lib/retoolkit-linux/full/`: Full-only relocatable private prefix.
- `/usr/share/applications/io.github.retoolkitlinux.REtoolkit.desktop`: alternative menu launcher.

The same relative layout is used inside AppDir and disposable test prefixes. No `/usr/bin/ghidra`, `/usr/bin/cutter`, `/usr/bin/radare2`, global Java configuration, repository entries or sudo startup commands are introduced.

Full tools are downloaded at build time, never in a maintainer script. `full-sources.json` pins exact official binary archives and SHA-256. `full-source-archives.json` pins source companions. Preserve upstream notices and source availability when redistributing. No public release is published by the build scripts.
