> Histórico da versão 0.1.0. Para Light/Full 0.2.0, consulte [VALIDATION-0.2.0.md](VALIDATION-0.2.0.md).

# Validação — 2026-10-23

Este documento separa resultados executados de funcionalidades implementadas e testes ainda pendentes. A versão 0.1.0 é funcional, mas **a missão completa ainda não está aceita**.

## Ambiente

Ubuntu 24.04.5 LTS x86_64, glibc 2.39, GTK 4.14.5, libadwaita 1.5.0. Rust/Cargo 1.99.1 em pasta local. Pacotes de desenvolvimento do sistema foram instalados pelo usuário após autorização. Nenhuma release foi publicada e nenhuma ferramenta foi instalada globalmente pelo agente.

## Resultados executados

| Verificação | Resultado |
|---|---|
| `cargo fmt --check` | Passou |
| `cargo clippy --all-targets -- -D warnings` | Passou |
| `cargo test` | 16 testes (15 de núcleo + 1 de lançador); passou |
| `cargo build --release` | Passou; ELF x86_64 real |
| `file` e `ldd` | ELF 64-bit PIE, sem bibliotecas ausentes no host |
| Interface X11 e Wayland | Janelas abertas sem root; smoke test passou nos dois backends; screenshot X11 inspecionado |
| Teste gráfico integrado | Inicialização/detecção, pesquisa YARA, análise de `/usr/bin/ls`, execução real de file em terminal e páginas configurações/operações passaram |
| Catálogo | 36 entradas, validação de schema e rejeição de comandos inválidos |
| Detecção inicial | GDB, strace, QEMU, file, strings, readelf, objdump, xxd e patchelf detectados no PATH |
| Inspeção ELF | `/usr/bin/ls`: SHA-256, arquitetura X86_64, entrypoint e seções reais |
| Arquivos hostis/malformados | Testes de ELF/PE truncados, arquivos grandes, não regulares, ZIP com caminho de traversal e argumentos literais passaram |
| Processos | Falha, timeout e descendente que herda stdout verificados |
| Instalação de ferramenta real | hashdeep 4.4-7.1 instalado por usuário em prefixo isolado `.build/validation/user-install`, SHA-256 oficial verificado, execução `-V` retornou 4.4 |
| Configuração | Roundtrip e erro em JSON inválido testados |
| Lançador | Criado em prefixo XDG descartável, validado por desktop-file-validate; recusa sobrescrita testada |
| DEB/RPM | Estrutura, metadata e instalação/remoção em prefixos privados via fakeroot; `--version` e smoke gráfico completo passaram nos dois pacotes; dependências do host reutilizadas |
| AppImage | Runtime/empacotador com SHA-256 conferido; `--version`, inspeção ELF e smoke gráfico completo com terminal passaram em modo extract-and-run |
| Operação offline | `--diagnose` observado com strace não abriu sockets AF_INET/AF_INET6; análise e testes do núcleo passaram sob rede restrita. Nenhuma transação offline de pacote foi certificada |

Os testes de pacote em prefixos privados usam `--force-depends` para DEB e `--nodeps`/relocação para RPM, pois não há um sistema completo dentro dessas pastas. **Não comprovam resolução de dependências ou instalação de sistema em Ubuntu limpo/Fedora.** Não execute esses flags em sua instalação real. O RPM não foi instalado pelo APT ou diretamente no sistema Ubuntu.

## Limitações e critérios ainda pendentes

- Instalação/remoção em VM limpa Ubuntu 24.04, Ubuntu 26.04 e Fedora; validação nativa Debian, Mint e Pop!_OS.
- Execução AppImage por montagem FUSE: `/dev/fuse` não está disponível neste ambiente. O fallback de extração foi testado.
- Transação APT/DNF real por polkit: sem autorização administrativa interativa disponível ao agente. Simulação APT e instalação privada real são os fluxos verificáveis aqui.
- DNF e Flatpak têm backend, mas não foram validados com instalação/atualização/remoção reais. DNF ainda não mostra uma resolução completa de dependências na prévia.
- Atualizações da própria aplicação: canal oficial, changelog remoto e download verificado ainda não configurados. Não há atualização automática ou silenciosa.
- Automação de instalações oficiais de Ghidra/Cutter/Frida e demais ferramentas sem pacote suportado continua manual. O backend genérico de binários com hash é infraestrutura, não um fluxo certificado.
- Firmware: identificação inicial; não há extração de sistemas de arquivos, emulação ou sandbox. DEX não tem decompilação e APK não decodifica AndroidManifest.xml.
- Plugins GDB não são detectados como extensões carregadas. Versões externas sem probe confiável são exibidas como não detectadas.
- Logs persistentes ainda não possuem rotação automática; progresso gráfico é texto/progresso indeterminado, sem percentual inventado.
- Atalho real GNOME e suas regras de confiança precisam de verificação manual do usuário; o teste de lançador foi isolado, sem criar atalho no desktop real.
- CI criada, mas sem execução em GitHub nesta sessão; não houve publicação de release.

## Evidências

Os logs e relatórios locais estão em `.build/validation/`; um conjunto final acompanha `dist/validation/`. O código de teste gráfico embutido usa widgets GTK reais e o mesmo subprocesso de análise da interface. Ele não automatiza o diálogo nativo de seleção, drag-and-drop físico ou o agente polkit.

Avisos não fatais observados: aceleração EGL/DRI3 indisponível em Xvfb (renderização Cairo utilizada), módulos GTK locais opcionais ausentes e portais GNOME sem acesso FUSE. Uma primeira tentativa de GUI dentro de fakeroot falhou na autenticação D-Bus; o script foi corrigido para abrir a GUI fora da emulação, e os dois pacotes passaram.
