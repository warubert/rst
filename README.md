# RST (Rust Subtitle Tools)

Ferramenta de terminal escrita em Rust para extrair legendas de arquivos de vídeo e salvá-las em formato `.srt`.

O RST usa `ffprobe` para detectar as faixas de legenda e `ffmpeg` para exportar a faixa selecionada.

## Funcionalidades

- Detecta as faixas de legenda disponíveis em um arquivo de vídeo
- Permite selecionar a faixa a extrair
- Salva a legenda selecionada em formato `.srt`
- Oferece uma interface interativa de terminal com Ratatui
- Disponibiliza a interface em português, inglês e espanhol
- Detecta o idioma do sistema na primeira execução e permite alterá-lo em **Opções**
- Usa inglês quando o idioma do sistema não é português, inglês ou espanhol

O idioma escolhido em **Opções** é salvo nas configurações do usuário e tem prioridade sobre o idioma do sistema nas próximas execuções. As opções **Traduzir legenda** e **Extrair e traduzir** ainda não estão implementadas.

## Requisitos

É necessário ter o FFmpeg instalado e os comandos `ffmpeg` e `ffprobe` disponíveis no `PATH`:

- Linux: normalmente via gerenciador de pacotes (`sudo apt install ffmpeg`, `sudo dnf install ffmpeg`, etc.)
- macOS: `brew install ffmpeg`
- Windows: instalar FFmpeg e garantir que `ffmpeg` e `ffprobe` fiquem no PATH

Também é necessário ter o Rust e o Cargo instalados para compilar o projeto.

## Testes

Execute a suíte automatizada com:

```bash
cargo test
```

Os testes cobrem o parsing das faixas de legenda, os argumentos usados na extração, o fluxo de entrada com e sem argumento, as ações do menu, a geração do nome `.srt`, a seleção e persistência do idioma e a renderização das telas Ratatui.

Para verificar formatação, compilação e lint:

```bash
cargo fmt --check
cargo check
cargo clippy -- -D warnings
```

## Preferência de idioma

O idioma selecionado é gravado no arquivo `rst/language` dentro do diretório de configuração do usuário:

- Linux: `~/.config/rst/language` (ou `$XDG_CONFIG_HOME/rst/language`, se definido)
- macOS: `~/Library/Application Support/rst/language`
- Windows: `%APPDATA%\rst\language`

## Compilar

```bash
cargo build --release
```

O executável será criado em `target/release/rst`.

## Uso

Inicie sem argumentos para abrir o menu:

```bash
cargo run
```

Use as setas para navegar e Enter para selecionar. Escolha **Extrair legenda** para informar o caminho do vídeo, selecionar uma faixa e definir o nome do arquivo `.srt` de saída.

Também é possível informar o arquivo de vídeo como argumento. Nesse caso, o menu inicial é ignorado e o programa segue diretamente para a seleção da legenda:

```bash
cargo run -- "video.mkv"
```

## Extração

O comando FFmpeg usado para extrair a faixa selecionada segue este formato:

```bash
ffmpeg -i arquivo.mkv -map 0:INDICE_DA_FAIXA arquivo_saida.srt
```

Se o nome de saída não for informado, o programa usa o nome do arquivo de vídeo com a extensão `.srt`.
