# Desenvolvimento

Rust estável ≥ 1.88 é necessário somente para desenvolvimento. Nesta máquina existe toolchain local em `.build`; `scripts/env.sh` o seleciona. Em um clone novo, instale um Rust estável compatível pelo método oficial. Dependências de build incluem GTK4/libadwaita, pkg-config, GCC, RPM, fakeroot, desktop-file-utils, Xvfb, xauth, x11-utils, ImageMagick e Python 3.12.

```bash
source scripts/env.sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 -m unittest discover -s tests -p 'test_*.py'
cargo build --release

python3 scripts/fetch-full.py
python3 scripts/prepare-full.py
python3 scripts/fetch-full-sources.py
scripts/package.sh                 # Light e Full em DEB/RPM
python3 scripts/fetch-appimage-tools.py
python3 scripts/appimage.py         # Light e Full em AppImage
scripts/check-packages.sh           # prefixos privados, sem instalar no host
python3 scripts/check-apt-editions.py # simulações de troca
python3 scripts/release-index.py    # índice, inventário e SHA-256
```

`--edition light` ou `--edition full` pode ser passado a `package.sh` e `appimage.py`. A Light não requer downloads das ferramentas da Full. A preparação do prefixo Full não sobrescreve um prefixo de versão diferente: mova o anterior explicitamente antes de reconstruir.

Manifestos de conteúdo: `packaging/editions/profiles.json`. URLs/versões/checksums oficiais: `full-sources.json`. Arquivos-fonte redistribuídos e checksums: `full-source-archives.json`. Alterações de versão devem ser revisadas e os hashes atualizados explicitamente.

Diagnósticos:

```bash
retoolkit-linux --version
retoolkit-linux --edition
retoolkit-linux --diagnose
retoolkit-linux --inspect /usr/bin/ls
xvfb-run -a dbus-run-session -- env GDK_BACKEND=x11 GSK_RENDERER=cairo retoolkit-linux --gui-smoke-test
xvfb-run -a python3 scripts/check-full-tools.py --gui
```

Arquitetura: GTK em `src/ui`; catálogo, detecção, instalações, análise, processos, edição e configuração separados. Manifestos e estilos do aplicativo são embutidos no ELF; a identidade e o prefixo de ferramentas vêm dos dados da edição instalada. A CI gera as duas edições como artefatos separados e não publica GitHub Releases automaticamente.

