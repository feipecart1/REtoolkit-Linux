## Escolha sua edição

**Light:** aplicativo nativo com dashboard, catálogo de 36 ferramentas, detecção e análise estática. Instale ferramentas sob demanda.

**Full:** inclui Ghidra 12.2, Cutter 2.5.0, radare2 6.2.4 e Java 25. O ícone recebe um raio dourado para identificar a edição.

| Formato | Light | Full |
|---|---|---|
| Ubuntu 24.04 x86_64 | `retoolkit-linux-light_0.2.0_amd64.deb` | `retoolkit-linux-full_0.2.0_amd64.deb` |
| AppImage x86_64 | `REtoolkit-Linux-Light-0.2.0-x86_64.AppImage` | `REtoolkit-Linux-Full-0.2.0-x86_64.AppImage` |
| RPM experimental | `retoolkit-linux-light-0.2.0.x86_64.rpm` | `retoolkit-linux-full-0.2.0.x86_64.rpm` |

## Instalar no Ubuntu

Baixe **uma** edição e execute na pasta do download:

```bash
sudo apt install ./retoolkit-linux-full_0.2.0_amd64.deb
# Ou escolha a Light:
sudo apt install ./retoolkit-linux-light_0.2.0_amd64.deb
```

Abra pelo menu ou execute `retoolkit-linux`, sem sudo. Os pacotes Light e Full substituem um ao outro.

Na Full DEB/RPM, o gerenciador nativo também resolve as ferramentas básicas declaradas. A Full AppImage inclui os três programas principais e Java; as ferramentas básicas do sistema permanecem sob demanda.

Para AppImage, dê permissão de execução com `chmod +x NOME.AppImage`. Se FUSE não estiver disponível, use `APPIMAGE_EXTRACT_AND_RUN=1 ./NOME.AppImage`.

**Compatibilidade:** Ubuntu 24.04 x86_64; glibc ≥ 2.39. Fedora/RPM e outras distribuições ainda precisam de validação em sistema limpo. Rust/Cargo não são necessários para usar o aplicativo.

## Integridade e fontes

Os anexos incluem `SHA256SUMS`, inventário de conteúdo, licença e fontes correspondentes de REtoolkit, Cutter/Rizin, radare2 e OpenJDK. As ferramentas incluídas preservam suas licenças originais. Para verificar os arquivos baixados:

```bash
sha256sum --ignore-missing -c SHA256SUMS
```
