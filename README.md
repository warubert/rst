# rst

Extrator de legendas em Rust.

Este projeto usa ffprobe para detectar as streams de legenda de um arquivo de vídeo e ffmpeg para exportar a legenda selecionada em formato `.srt`.

## Funcionalidades

- Detecta automaticamente as legendas disponíveis no arquivo
- Exibe as opções numeradas no terminal
- Permite escolher a legenda pelo número da opção
- Se o usuário pressionar Enter sem digitar nada, usa a primeira opção por padrão
- Solicita o nome do arquivo de saída, com valor padrão igual ao nome do vídeo original
- Acrescenta automaticamente `.srt` ao nome informado

## Requisitos

Instale o FFmpeg no sistema:

- Linux: normalmente via gerenciador de pacotes (`sudo apt install ffmpeg`, `sudo dnf install ffmpeg`, etc.)
- macOS: `brew install ffmpeg`
- Windows: instalar FFmpeg e garantir que `ffmpeg` e `ffprobe` fiquem no PATH

## Como usar

```bash
cargo run -- AloneAustraliaS02E05.mkv
```

O programa vai:

1. analisar o arquivo com `ffprobe`
2. listar as legendas disponíveis
3. pedir a opção desejada
4. pedir o nome do arquivo de saída
5. exportar a legenda em `.srt`

## Exemplo de interação

```text
Opcoes de linguas encontradas:
- 1: en
- 2: pt-BR (SDH)

Digite o numero da opcao desejada (padrao: 1):

Digite o nome da legenda de saida (padrao: AloneAustraliaS02E05.srt):
```

Se o usuário apertar Enter nas duas perguntas, o programa usa:

- legenda 1
- arquivo de saída: `AloneAustraliaS02E05.srt`

## Build

```bash
cargo build
```

## Observação

O comando final executado segue a lógica:

```bash
ffmpeg -i arquivo.mkv -map 0:[indice_selecionado] arquivo_saida.srt
```
