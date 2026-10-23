# Validação 0.2.0 — Light / Full — 2026-10-23

## Escopo e ambiente

Ubuntu 24.04.5 LTS x86_64, glibc 2.39, GTK 4.14.5 e libadwaita 1.5.0; Rust 1.99.1 local. Artefatos preparados em `dist/light/` e `dist/full/`. Não houve instalação global da Full pelo agente nem publicação de release. As duas edições compartilham o aplicativo e a configuração XDG, mas têm conteúdo e identidade de instalação distintos.

## Executado

| Verificação | Resultado |
|---|---|
| `cargo fmt --check` | Passou |
| `cargo clippy --all-targets -- -D warnings` | Passou |
| `cargo test` | 20 testes passaram: 4 de edição, 1 de lançador e 15 do núcleo |
| Contratos Python das edições | 4 testes passaram: Light sem ferramentas, Full com os três programas, pins oficiais e relação entre pacotes |
| Release Rust | ELF x86_64 compilou; edição/versão consultáveis por CLI |
| Download das ferramentas | SHA-256 oficial conferido para Ghidra, Cutter, radare2 e JDK |
| Java incluído | `java -version` executou Temurin 25.0.5 |
| radare2 incluído | 6.2.4 executou e inspecionou estaticamente `/usr/bin/ls` como ELF 64-bit |
| Cutter incluído | Consulta retornou 2.5.0 e a janela de primeiro uso ficou visível em Xvfb |
| Ghidra incluído | Launcher headless apresentou a interface de uso com Java incluído; janela do acordo de primeiro uso ficou visível. Nenhum acordo foi aceito automaticamente |
| DEB/RPM Light e Full | Instalação, identificação da edição, detecção do prefixo privado e remoção passaram em diretórios descartáveis sob fakeroot |
| GUI dos DEBs | Light e Full passaram em inicialização/detecção, pesquisa YARA, análise ELF, execução real de `file` em terminal e páginas de configurações/operações |
| APT Light/Full | Simulação no estado real do host resolveu as dependências e a substituição da antiga versão 0.1.0 |
| Troca Light → Full / Full → Light | Os dois planos de substituição passaram com cópias privadas do status APT; o estado Full inclui as dependências que o APT instalaria |
| AppImage Light e Full | Ambos identificaram corretamente a edição e passaram no smoke gráfico completo em modo extract-and-run |
| Conteúdo do Full AppImage | Diagnóstico real encontrou os lançadores Ghidra/Cutter/radare2 dentro do AppDir e identificou o Java incluído |
| Fontes e avisos | Arquivos-fonte oficiais de Cutter/Rizin, radare2 e JDK baixados com SHA-256; avisos originais e dependências Cargo preservados |

Os testes de pacotes usam `--force-depends` no prefixo DEB e `--nodeps`/relocação no RPM. Bibliotecas do host são reutilizadas e dependências nativas não são instaladas dentro desses prefixos. Portanto, não são testes de um sistema limpo. A simulação APT verifica a resolução das dependências, mas não executa a transação administrativa.

A Full AppImage contém os três programas e Java; os extras declarados como dependências APT/DNF são exclusivos dos pacotes nativos. Light foi verificada sem prefixo Full. O inventário por formato consta em `dist/release-manifest.json`.

## Pendências e limites

- Instalação administrativa real de cada edição e troca entre elas em VM limpa Ubuntu 24.04/26.04.
- Instalação/remoção nativa Fedora e resolução de dependências dos RPMs nessa distribuição. RPMs permanecem experimentais.
- Montagem FUSE do AppImage: `/dev/fuse` não está disponível. O fallback de extração foi executado.
- Revisão completa de licenças/fontes de todas as dependências internas das distribuições de terceiros antes de publicar uma release pública. As licenças originais não foram substituídas por MIT; fontes principais e referências acompanham a Full.
- Ghidra/Cutter tiveram a inicialização e suas janelas de primeiro uso verificadas; não houve aceitação automática de acordo, projeto de análise completo ou teste de todos os plugins.
- Cutter usa Qt/XCB na distribuição escolhida, podendo depender de XWayland. O novo teste gráfico de edição foi feito em X11/Xvfb; a GUI principal já havia passado em Wayland na validação 0.1.0.
- Canal de atualização da aplicação, transações reais DNF/Flatpak e certificação em outras distribuições continuam pendentes, como descrito em `VALIDATION.md`.
- A Full é uma seleção explícita, não as 36 ferramentas. Ferramentas comerciais ou sem instalador suportado continuam com distribuição/licença próprias.

## Evidências

Logs em `.build/editions/`, testes de ferramentas em `.build/full-tool-check.*`, testes de migração em `.build/apt-editions.*` e transações de pacote em `.build/check-light.*` / `.build/check-full.*`. Um conjunto final acompanha `dist/validation-0.2.0/`.

Avisos não fatais: EGL/DRI3 indisponível no Xvfb, módulos GTK opcionais locais ausentes e portais GNOME sem FUSE. A criação do primeiro DEB Full foi interrompida durante compressão; o empacotamento foi corrigido para zstd nível 3 com dois workers e saída temporária promovida somente após sucesso. O artefato final foi instalado, executado e removido nos testes de prefixo privado.
