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
- Traduz arquivos `.srt` pelo serviço de tradução do Google, dividindo legendas grandes em várias requisições assíncronas

O idioma escolhido em **Opções** é salvo nas configurações do usuário e tem prioridade sobre o idioma do sistema nas próximas execuções. A opção **Extrair e traduzir** ainda não está implementada.

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

## Tradução

Escolha **Traduzir legenda** no menu. O programa pergunta, em ordem:

1. o caminho do arquivo `.srt`;
2. o idioma original da legenda (padrão `auto`, detecção automática);
3. o idioma de destino (padrão: o idioma configurado no programa);
4. o nome do arquivo traduzido (padrão: `nome_original-idioma.srt`, por exemplo `AloneS13E13-pt.srt`).

Os tempos e as tags de posição (como `{\an8}`) são preservados. As legendas são enviadas em lotes de até 100 textos ou 4000 caracteres, com até 4 requisições simultâneas e novas tentativas em caso de falha.

### Chave de API

O endpoint rejeita requisições sem uma chave, e a chave não faz parte do código para não ser versionada. O programa a lê da variável de ambiente `GOOGLE_TRANSLATE_API_KEY`, que pode ser definida no ambiente ou em um arquivo `.env` na pasta onde o programa é executado (carregado automaticamente).

1. Copie o arquivo de exemplo e preencha a chave:

   ```bash
   cp .env.example .env
   ```

   ```env
   GOOGLE_TRANSLATE_API_KEY=sua_chave_aqui
   ```

2. O `.env` está no `.gitignore` e não deve ser commitado. O `.env.example` é versionado apenas como modelo, sem a chave real.

Para obter uma chave:

1. Acesse o [Google Cloud Console](https://console.cloud.google.com/) e crie ou selecione um projeto.
2. Em **APIs e serviços**, ative a **Cloud Translation API**.
3. Em **APIs e serviços > Credenciais**, escolha **Criar credenciais > Chave de API**.
4. Restrinja a chave à Cloud Translation API e copie o valor para o `.env`.

Sem a chave, a opção **Traduzir legenda** exibe um aviso e não pede os dados da tradução. Como o endpoint usado é não oficial, o Google pode alterar ou bloquear seu uso, e uma chave criada pelo Console pode não ser aceita por ele.

O teste com o serviço real é ignorado por padrão e usa a chave do `.env`:

```bash
cargo test translates_the_sample_file_with_the_real_service -- --ignored
```

### Em desenvolvimento

- testar metodo de tradução sem api key
- setar chave de api pelas opçoes
- fluxo unico extração + tradução
- remoção do uso do ffprobe e ffmpeg para alg próprio
- criação de legendas com whisper
- modo terminal com args completo
- compilar executavel para diversas arquiteturas
- possibilidade de interface grafica
