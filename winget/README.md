# Manifesto Winget para WNCDU

Este diretório contém os arquivos de manifesto oficiais para publicação do **WNCDU** no **Windows Package Manager (Winget)**.

## Identificador do Pacote
- **PackageIdentifier**: `lpl2103.wncdu`
- **Versão**: `0.1.0`
- **Tipo de Instalador**: `portable` (executável único standalone)
- **Comando**: `wncdu`

## Estrutura dos Arquivos
- `manifests/l/lpl2103/wncdu/0.1.0/lpl2103.wncdu.yaml` — Versão do manifesto
- `manifests/l/lpl2103/wncdu/0.1.0/lpl2103.wncdu.installer.yaml` — URL e SHA256 do instalador portátil
- `manifests/l/lpl2103/wncdu/0.1.0/lpl2103.wncdu.locale.en-US.yaml` — Metadados em inglês
- `manifests/l/lpl2103/wncdu/0.1.0/lpl2103.wncdu.locale.pt-BR.yaml` — Metadados em português

## Validação Local
Os manifestos foram validados usando o próprio CLI do Winget:
```powershell
winget validate --manifest winget/manifests/l/lpl2103/wncdu/0.1.0
```

## Como Submeter para o repositório oficial da Microsoft (`microsoft/winget-pkgs`)

### Opção 1: Via script automatizado (recomendado)
Execute no PowerShell:
```powershell
.\winget\submit_to_winget.ps1
```
O script fará o fork do repositório `microsoft/winget-pkgs`, criará o branch, copiará os manifestos e abrirá o Pull Request automaticamente usando o `gh` CLI.

### Opção 2: Via `wingetcreate`
Instale a ferramenta oficial da Microsoft e envie:
```powershell
winget install Microsoft.WingetCreate
wingetcreate submit winget/manifests/l/lpl2103/wncdu/0.1.0
```
