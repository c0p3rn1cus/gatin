# Gatin 🐈‍⬛

Um gatinho preto **desktop pet** escrito 100% em **Rust**, rodando de forma nativa e transparente no **Linux (Wayland & X11)**, **Windows** e **macOS**.

## Funcionalidades

- **Janela flutuante transparente, sem bordas e sempre no topo** (`always-on-top`)
- **Animação em tempo real do teclado e mouse**:
  - As patas articuladas batem exatamente nas teclas pressionadas no mini teclado mecânico 60% (com destaque em verde-limão nas teclas pressionadas e detalhes em laranja)
  - A pata direita alcança o mouse branco quando você mexe o cursor e ilumina o mouse ao clicar
  - **Teclinhas brancas flutuantes** (`S`, `E`, `A`, `⇧`, `↵`, `⌫`, espaço, etc.) sobem acima do teclado enquanto você digita
- **Carinho no gato**: passe o mouse sobre a cabeça/corpo do gatinho — o cursor vira mãozinha e ele fecha os olhos em fendas verdes felizes e inclina a cabeça!
- **Rastreamento de olhar & piscadas naturais**: os olhos acompanham o cursor e o teclado
- **Modo privacidade automático (campos de senha)**: quando você foca qualquer campo de senha (no navegador, diálogos de autenticação ou `sudo`/`ssh` no terminal), o gatinho **tampa os olhos com as duas patinhas** e não mostra nenhuma tecla nem ilumina o teclado!
  - **Linux**: usa `AT-SPI2` (`libatspi`) + detecção de diálogos polkit/pinentry e prompts sem eco (`ICANON && !ECHO`) em terminais.
    - *Nota para Chromium/Brave no Linux (`at-spi2-core` recente)*: o Chromium só exporta os campos `<input type="password">` de páginas web para o AT-SPI2 se `--force-renderer-accessibility` estiver ativo (adicione `--force-renderer-accessibility` em `~/.config/chromium-flags.conf` e reinicie o navegador). No Firefox funciona direto.
  - **Windows**: usa `UI Automation` (`CurrentIsPassword`).
  - **macOS**: usa `IsSecureEventInputEnabled` (`Carbon`).
- **Interação**:
  - **Arrastar com botão esquerdo**: move o gatinho pela tela
  - **Scroll do mouse**: redimensiona o gatinho (`50%` a `250%`)
  - **Botão direito**: abre o menu (Sempre no topo, Teclas flutuantes, Modo Demo, Click-through, Tamanho, Sair)

## Download pronto (Windows, macOS e Linux)

Baixe o pacote compilado para o seu sistema na página de [**Releases**](https://github.com/c0p3rn1cus/gatin/releases/latest):

- **Windows (x86_64)**: `gatin-windows-x86_64.zip` → extraia e execute `gatin.exe`
- **macOS (Apple Silicon & Intel — Universal)**: `gatin-macos-universal.dmg` (ou `.zip`) → arraste `Gatin.app` para *Aplicativos*
- **Linux (x86_64)**: `gatin-linux-x86_64.tar.gz` → extraia e execute `./gatin`

## Como compilar do código-fonte

Requisito: [Rust toolchain](https://rustup.rs/) (`rustc` / `cargo`).

```bash
cargo run --release
```

Para testar imediatamente com digitação, mouse e carinho automáticos:

```bash
cargo run --release -- --demo
```

Outras opções de linha de comando:

```bash
gatin --scale 1.25       # tamanho inicial
gatin --no-keycaps       # oculta as teclas flutuantes
gatin --no-top           # desativa "sempre no topo"
gatin --click-through    # cliques atravessam a janela
```

## Permissões por sistema operacional

### Linux (Wayland / Hyprland / Omarchy / Sway / GNOME / KDE)
O `gatin` usa **`evdev`** diretamente no Linux para funcionar tanto em **Wayland** quanto em **X11** (com fallback automático para `libX11` no X11).

Para capturar teclado e mouse globalmente no Wayland, adicione seu usuário ao grupo `input` (uma única vez) e faça logout/login:

```bash
sudo usermod -aG input $USER
```

#### Dica para Hyprland / Omarchy
Para garantir janela flutuante, fixada em todos os workspaces, sem bordas, sem blur e sem sombra no Hyprland, adicione em `~/.config/hypr/hyprland.conf` (ou nas suas windowrules):

```ini
windowrule = float on, match:class ^(gatin)$
windowrule = pin on, match:class ^(gatin)$
windowrule = border_size 0, match:class ^(gatin)$
windowrule = no_shadow on, match:class ^(gatin)$
windowrule = no_blur on, match:class ^(gatin)$
```

### Windows
Funciona direto usando hooks nativos Win32 (`WH_KEYBOARD_LL` e `WH_MOUSE_LL`), sem precisar de permissões especiais.

```powershell
cargo build --release
.\target\release\gatin.exe
```

### macOS (Apple Silicon & Intel)
Usa `CoreGraphics` (`CGEventTap` em modo *listen-only*).

Na primeira vez, conceda permissão em:
**Ajustes do Sistema → Privacidade e Segurança → Monitoramento de Entrada (Input Monitoring)** (e/ou **Acessibilidade**) para o terminal ou binário `gatin`, e reabra o app.

```bash
cargo build --release
./target/release/gatin
```
