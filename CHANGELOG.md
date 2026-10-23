# Changelog

## 0.2.1

- Ferramentas CLI pedem argumentos antes de executar, com orientação para strace, ltrace e YARA.
- Impede abrir ferramentas que exigem um alvo com uma linha de comando vazia.
- Mantém argumentos estruturados e caminhos entre aspas, sem executar um shell.


## 0.1.0 — 2026-10-23

- Aplicação GTK4/libadwaita nativa em Rust, com dashboard, navegação e pesquisa.
- Catálogo versionado de 36 ferramentas com detecção real e metadados de licença/origem.
- Planejamento APT com simulação, confirmação, polkit e logs; backends DNF e Flatpak iniciais.
- Instalação privada de hashdeep com pacote Ubuntu oficial, HTTPS, SHA-256 e validação de versão.
- Lançamento GUI/CLI sem shell; histórico e configuração XDG persistente.
- Inspeção estática limitada de ELF, PE, DEX e ZIP/APK em subprocesso; identificação de firmware.
- Instalador gráfico por usuário e lançador XDG com atalho de desktop opcional.
- ELF, DEB, RPM e AppImage, scripts de empacotamento e CI sem publicação automática.
- Validação local Ubuntu 24.04; certificação de distribuições e canal de atualização da aplicação pendentes.

## 0.2.0 — 2026-10-23

- Edições alternativas **Light** e **Full**, com identidade visível e inventário consultável por `--edition`.
- Full inclui Ghidra 12.2, Cutter 2.5.0, radare2 6.2.4 e Java 25 oficiais em prefixo privado e relocável, com SHA-256 fixado.
- Full DEB/RPM declara ferramentas básicas como dependências da mesma transação; sem downloads em postinst ou APT aninhado.
- Full AppImage inclui os três programas principais e Java; ferramentas básicas adicionais continuam sob demanda nesse formato.
- Pacotes declaram alternativas e conflitos; configuração XDG compartilhada é preservada.
- Light não inclui ferramentas de terceiros; radare2 pode ser instalado sob demanda via APT.
- Avisos de licença preservados e arquivos-fonte oficiais preparados junto à Full.
