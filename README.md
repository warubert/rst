# rst

Extrator de legendas em Rust.

Este projeto usa ffprobe para detectar as streams de legenda de um arquivo de vídeo e ffmpeg para exportar a legenda selecionada em formato `.srt`.

## Funcionalidades

- Detecta automaticamente as legendas disponíveis no arquivo

## Requisitos

Instale o FFmpeg no sistema:

- Linux: normalmente via gerenciador de pacotes (`sudo apt install ffmpeg`, `sudo dnf install ffmpeg`, etc.)
- macOS: `brew install ffmpeg`
- Windows: instalar FFmpeg e garantir que `ffmpeg` e `ffprobe` fiquem no PATH

## Como usar

```bash
cargo run -- AloneAustraliaS02E05.mkv
```

## Build

```bash
cargo build
```

## Observação

O comando final executado segue a lógica:

```bash
ffmpeg -i arquivo.mkv -map 0:[indice_selecionado] arquivo_saida.srt
```
