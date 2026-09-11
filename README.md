# WNCDU

[![CI](https://github.com/lpl2103/wncdu/actions/workflows/ci.yml/badge.svg)](https://github.com/lpl2103/wncdu/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/lpl2103/wncdu?include_prereleases&style=flat-square)](https://github.com/lpl2103/wncdu/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](LICENSE)

> **WNCDU** é um analisador de uso de disco interativo e de alto desempenho para Windows (e Linux), desenvolvido em Rust com interface TUI via `ratatui`.
> Inspirado no clássico [ncdu](https://dev.yorhel.nl/ncdu) e no [zig-ncdu](https://github.com/ra9fael/zig-ncdu), com foco total nas particularidades do Windows.

---

## ⚡ Destaques e Diferenciais

- **Scanner Win32 Nativo**: Utiliza chamadas diretas `FindFirstFileW`/`FindNextFileW` que retornam nome, tamanho, atributos e timestamps em uma única syscall, dispensando chamadas `stat` redundantes.
- **Suporte a Caminhos Longos**: Prefixo verbatim `\\?\` automático para contornar a limitação de 260 caracteres do Windows (`MAX_PATH`).
- **Respeito a Reparse Points**: Detecção automática de Junctions e Symlinks do Windows para evitar loops infinitos ou contagem duplicada.
- **Árvore na Memória em Arena Flat**: Armazenamento contíguo em `Vec<DirEntry>` com índices, garantindo máxima localidade de cache e eliminando fragmentação de heap.
- **Nomes Inlinados na Stack**: `CompactString` inlina nomes de arquivos até 24 caracteres na stack sem tocar no heap.
- **Binário Único Estático**: Compilado com LTO completo, CRT estático (`target-feature=+crt-static`) e remoção de símbolos de debug para binários compactos e sem dependências externas.
- **Interface TUI Rica**: Renderizado com `ratatui`, barras proporcionais clássicas (`#`) e blocos Unicode, métricas em tempo real e modais de ajuda, informações e confirmação de exclusão.
- **Ícone e Metadados Nativos**: Ícone integrado no executável PE para exibição no Windows Explorer e na barra de tarefas.

---

## 📦 Instalação

### Via Windows Package Manager (Winget)

```powershell
winget install lpl2103.wncdu
```

### Download Direto

Baixe o executável pré-compilado `wncdu.exe` na aba [Releases](https://github.com/lpl2103/wncdu/releases/latest) e coloque-o em um diretório presente no seu `PATH`.

### Via Cargo

```bash
cargo install --git https://github.com/lpl2103/wncdu
```

---

## ⌨️ Atalhos de Teclado (Paridade com ncdu)

| Tecla | Ação |
|---|---|
| `↑`, `k` | Mover cursor para cima |
| `↓`, `j` | Mover cursor para baixo |
| `Enter`, `→`, `l` | Entrar no diretório selecionado |
| `Backspace`, `←`, `h` | Subir para o diretório pai |
| `Home`, `g` | Pular para o primeiro item |
| `End`, `G` | Pular para o último item |
| `PgUp`, `PgDn` | Rolar uma página para cima/baixo |
| `s` | Ordenar por tamanho (alterna cresc/decresc) |
| `n` | Ordenar por nome (alterna A-Z / Z-A) |
| `c`, `C` | Ordenar por contagem de itens |
| `m`, `M` | Ordenar por data de modificação |
| `a` | Alternar entre tamanho aparente e tamanho em disco |
| `t` | Alternar agrupamento de diretórios no topo |
| `e` | Mostrar / ocultar arquivos e pastas ocultos |
| `p` | Mostrar / ocultar coluna de porcentagem relativa |
| `u`, `v` | Mostrar / ocultar barra de gráfico de uso |
| `b` | Alternar estilo do gráfico (`[###]` vs `████`) |
| `i` | Exibir informações detalhadas do item selecionado |
| `d` | Deletar arquivo ou pasta selecionada (com confirmação) |
| `?` | Abrir janela de ajuda |
| `q`, `Ctrl+C` | Sair do programa |

---

## 🚀 Uso da Linha de Comando

```bash
# Analisar a pasta atual
wncdu

# Analisar uma pasta ou unidade específica
wncdu C:\Users\Leandro
wncdu D:\

# Não cruzar partições/drives
wncdu -x C:\

# Excluir pastas ou padrões
wncdu --exclude "*.tmp" --exclude "node_modules"

# Excluir pastas de cache com CACHEDIR.TAG
wncdu --exclude-caches

# Usar unidades decimais (SI: KB, MB, GB) em vez de binárias (KiB, MiB, GiB)
wncdu --si
```

---

## 🛠️ Compilação

### Requisitos
- Rust 1.85+
- MinGW-w64 (WinLibs) ou MSVC

```bash
# Build de debug
cargo build

# Rodar os testes
cargo test

# Build de release ultra-otimizado (binário estático sem CRT dependente)
cargo build --release
```

O binário final será gerado em `target/release/wncdu.exe`.

---

## 📄 Licença
Distribuído sob a licença [MIT](LICENSE).
