# RST (Rust Subtitle Tools)

Ferramenta de terminal escrita em Rust para extrair legendas de arquivos de vídeo e salvá-las em formato `.srt`.

O RST detecta e extrai faixas de legenda de arquivos MKV/WebM e MP4/MOV com parsers Rust integrados.

## Funcionalidades

- Detecta as faixas de legenda disponíveis em um arquivo de vídeo
- Permite selecionar a faixa a extrair
- Salva a legenda selecionada em formato `.srt`
- Oferece uma interface interativa de terminal com Ratatui
- Permite navegar por pastas e selecionar arquivos de vídeo e `.srt` na interface
- Disponibiliza a interface em português, inglês e espanhol
- Detecta o idioma do sistema na primeira execução e permite alterá-lo em **Opções**
- Usa inglês quando o idioma do sistema não é português, inglês ou espanhol
- Traduz arquivos `.srt` pelo serviço de tradução do Google, dividindo legendas grandes em várias requisições assíncronas
- Extrai e traduz uma faixa de legenda do vídeo em um único fluxo
- Cria legendas `.srt` com Whisper a partir do áudio do vídeo
- Cria legendas com Whisper e traduz o resultado em um único fluxo

O idioma escolhido em **Opções** é salvo nas configurações do usuário e tem prioridade sobre o idioma do sistema nas próximas execuções.

## Requisitos

Também é necessário ter o Rust e o Cargo instalados para compilar o projeto.

Para criar legendas com Whisper, instale o [OpenAI Whisper](https://github.com/openai/whisper) e deixe os comandos `whisper` e `ffmpeg` disponíveis no `PATH` (por exemplo, `pip install -U openai-whisper` e `sudo apt install ffmpeg`). Essa opção fica indisponível se o comando Whisper não puder ser executado; o FFmpeg é usado pelo Whisper para ler o áudio.

## Testes

Execute a suíte automatizada com:

```bash
cargo test
```

Os testes cobrem o parsing e a extração de faixas de legenda, a conversão para SRT, a transcrição, o progresso do Whisper, o fluxo de entrada com e sem argumento, as ações do menu, a geração do nome `.srt`, a seleção e persistência do idioma, a renderização das telas Ratatui e o fluxo combinado de extração e tradução com dependências simuladas.

Para verificar formatação, compilação e lint:

```bash
cargo fmt --check
cargo check
cargo clippy -- -D warnings
```

## Configurações do usuário

As configurações são gravadas dentro do diretório de configuração do usuário:

- Linux: `~/.config/rst/` (ou `$XDG_CONFIG_HOME/rst/`, se definido)
- macOS: `~/Library/Application Support/rst/`
- Windows: `%APPDATA%\rst\`

O idioma fica no arquivo `language`; a chave digitada em **Opções** fica em `google_translate_api_key`; e o toggle **Usar chave de API** fica em `use_api_key`. A chave é armazenada em texto simples, mas sua tela de entrada sempre mostra `***`; no Unix, o arquivo tem permissão `0600`.

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

Use as setas para navegar e Enter para selecionar. Todas as telas da interface, exceto o menu principal, abrem em uma janela sobre ele. No explorador, digite para filtrar arquivos e pastas; Enter abre uma pasta ou seleciona um arquivo; Backspace apaga o filtro ou volta à pasta anterior; F2 permite digitar o caminho manualmente; Esc cancela. Durante a extração e a tradução, a interface mostra o progresso percentual. Escolha **Extrair legenda** para selecionar o vídeo, escolher uma faixa e definir o nome do arquivo `.srt` de saída.

Escolha **Extrair e traduzir** para extrair uma faixa do vídeo e traduzi-la em seguida. O idioma original é obtido dos metadados da faixa selecionada, então o programa pergunta apenas o idioma de destino. O SRT extraído é mantido junto com um segundo arquivo traduzido; é possível cancelar a tradução após a extração e manter o primeiro arquivo. Se os metadados não informarem o idioma, a detecção automática é usada.

Escolha **Criar legendas com Whisper** para selecionar o vídeo, escolher o idioma falado no autocomplete e definir o nome do `.srt` de saída. O nome sugerido por padrão mantém o nome-base do vídeo; o arquivo pode ser alterado ou receber outro caminho. O programa mostra o percentual informado pelo Whisper.

Escolha **Criar legendas com Whisper e traduzir** para gerar o SRT no idioma falado e traduzi-lo em seguida. O idioma selecionado no Whisper é usado como origem da tradução; o SRT original é preservado mesmo que a tradução seja cancelada ou falhe.

Sem argumentos, o programa abre o menu interativo. Para executar uma ação sem abrir a interface, use `--action` e informe os parâmetros exigidos por ela. Para extração pelo CLI, indique o código do idioma da faixa, como `pt` ou `en`; uma variante regional como `pt-BR` também corresponde a `pt`.

```bash
cargo run -- --action extract --input "video.mkv" --subtitle-language pt --output "video.srt"
cargo run -- --action translate --input "video.srt" --source-language en --target-language pt --output "video-pt.srt"
cargo run -- --action extract-and-translate --input "video.mkv" --subtitle-language en --target-language pt --output "video.srt" --translated-output "video-pt.srt"
cargo run -- --action transcribe --input "video.mkv" --language en --output "video.srt"
cargo run -- --action transcribe-and-translate --input "video.mkv" --language en --target-language pt --output "video.srt" --translated-output "video-pt.srt"
```

As ações aceitas são `extract`, `translate`, `extract-and-translate`, `transcribe` e `transcribe-and-translate`. Em ações combinadas, `--output` é o SRT original e `--translated-output` é o SRT traduzido. A tradução de uma faixa extraída usa o idioma dos metadados; quando ausente, usa detecção automática. `--source-language auto` também habilita detecção automática na ação `translate`. Consulte as opções com `cargo run -- --help`.

## Extração

São aceitas faixas textuais comuns em MKV/WebM (`S_TEXT/UTF8`, ASS/SSA e WebVTT) e faixas `mov_text` em MP4/MOV.

Se o nome de saída não for informado, o programa usa o nome do arquivo de vídeo com a extensão `.srt`.

## Tradução

Escolha **Traduzir legenda** no menu. O programa pergunta, em ordem:

1. o arquivo `.srt`, pelo explorador de arquivos ou digitando o caminho com `T`;
2. o idioma original em uma lista pesquisável, com **Automático** selecionado por padrão;
3. o idioma de destino, em uma lista pesquisável com os 249 idiomas do catálogo Google (nomes no idioma selecionado no programa; filtre por nome ou código; padrão: o idioma configurado no programa);
4. o nome do arquivo traduzido (padrão: `nome_original-idioma.srt`, por exemplo `AloneS13E13-pt.srt`).

Os tempos e as tags de posição (como `{\an8}`) são preservados. As legendas são enviadas em lotes de até 100 textos ou 4000 caracteres, com até 4 requisições simultâneas e novas tentativas em caso de falha.

### Chave de API (opcional)

O programa usa dois serviços de tradução. Em **Opções**, **Usar chave de API** controla qual será usado. Quando ativado, a chave salva nas opções tem prioridade sobre `GOOGLE_TRANSLATE_API_KEY`; se não houver chave, o programa usa o serviço sem chave. Quando desativado, o programa sempre usa o serviço sem chave.

| Situação                                 | Serviço                                 | Qualidade |
| ---------------------------------------- | --------------------------------------- | --------- |
| Toggle ligado e chave salva/env definida | `translateHtml`, com chave              | maior     |
| Toggle desligado ou nenhuma chave        | `translate_a/single` (`gtx`), sem chave | menor     |

Sem a chave a tradução funciona, e o programa avisa que está usando o serviço sem chave. Nos testes com o arquivo de exemplo, o resultado do serviço sem chave teve a mesma estrutura (legendas, tempos e tags), mas só cerca de 27% das legendas ficaram com o mesmo texto do serviço com chave; em vários casos a versão com chave soou mais natural.

Ela pode ser digitada em **Opções** ou fornecida pela variável de ambiente `GOOGLE_TRANSLATE_API_KEY` (inclusive por um arquivo `.env` na pasta onde o programa é executado). A chave salva nas opções tem prioridade sobre a variável de ambiente; desligar o toggle força o serviço sem chave.

1. Copie o arquivo de exemplo e preencha a chave:

   ```bash
   cp .env.example .env
   ```

   ```env
   GOOGLE_TRANSLATE_API_KEY=sua_chave_aqui
   ```

Para obter uma chave:

1. Acesse o [Google Cloud Console](https://console.cloud.google.com/) e crie ou selecione um projeto.
2. Em **APIs e serviços**, ative a **Cloud Translation API**.
3. Em **APIs e serviços > Credenciais**, escolha **Criar credenciais > Chave de API**.
4. Restrinja a chave à Cloud Translation API e copie o valor para o `.env`.

Os testes com o serviço real são ignorados por padrão. O primeiro usa a chave do `.env`; o segundo não precisa de chave:

```bash
cargo test translates_the_sample_file_with_the_real_service -- --ignored
cargo test translates_the_sample_file_without_an_api_key -- --ignored
```

### Em desenvolvimento

- tentar deixar mais bonito
- repo ubuntu e deb
- compilar executavel para diversas arquiteturas
