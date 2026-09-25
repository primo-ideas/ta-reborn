# Développement

## Commandes

| But | Commande |
|---|---|
| Tests (sim, protocole, relais) | `cargo test --workspace` |
| Lints | `cargo clippy --workspace --all-targets` |
| Client natif (menu) | `cargo run -p ta-client` |
| Client natif, solo direct | `cargo run -p ta-client -- solo` |
| Relais | `cargo run -p ta-server` (écoute `0.0.0.0:7878`, ou `-- ADRESSE:PORT`) |
| Client en ligne | `cargo run -p ta-client -- online [--server ws://HOTE:7878]`, à lancer deux fois |
### Web : reporté, rien de vérifié

Primo a reporté le web à plus tard. Le code prévu pour le web existe (`index.html` pour
trunk, `canvas: "#bevy"`, lecture de `?server=` dans `menu.rs`, `Connection` non-send)
mais **n'a jamais été compilé** pour `wasm32`. Commandes prévues :

| But | Commande |
|---|---|
| Compilation wasm | `cargo build -p ta-client --target wasm32-unknown-unknown` |
| Web, développement | `cd crates/ta-client && trunk serve`, puis http://127.0.0.1:8080 |
| Web, release | `cd crates/ta-client && trunk build --release`, sortie dans `crates/ta-client/dist/` |

Prérequis : `rustup target add wasm32-unknown-unknown` (déjà installé sur la machine de
Primo) et `cargo install --locked trunk` (pas installé). Sur le web, le relais par défaut
serait `ws://<hôte de la page>:7878`, modifiable avec `?server=ws://HOTE:PORT`. À
surveiller à la première compilation : les options de `getrandom` pour wasm.

## Vérifier un changement

1. `cargo test --workspace` et `cargo clippy --workspace --all-targets`, sans
   avertissement.
2. Changement visuel : lancer le binaire (`Start-Process target\debug\ta-client.exe
   -ArgumentList solo -PassThru`), capturer **sa fenêtre seulement** (script
   ci-dessous), puis regarder l'image avec Read.
3. Changement réseau : `crates/ta-server/tests/relay.rs` couvre le relais avec de vrais
   clients WebSocket. Bout en bout : `ta-server 127.0.0.1:7878` (adresse locale, pour
   éviter la fenêtre du pare-feu Windows) et deux clients
   `online --server ws://127.0.0.1:7878`. Vérifier dans le HUD : joueurs 1 et 2, même
   chronomètre, pas de « Desync ». Fermer un client doit afficher « Player 2 left » chez
   l'autre.

**Respect de la machine de Primo** : elle peut servir à autre chose pendant les tests,
avec une autre application au premier plan.
- Ne **jamais** capturer l'écran entier (c'est arrivé une fois) : capturer la fenêtre
  du processus uniquement.
- Ne **jamais** bouger le vrai curseur ni simuler clavier et souris au niveau du système.
  Envoyer des `WM_*` avec `PostMessage` à la fenêtre du jeu ne marche pas non plus :
  winit voit que le vrai curseur n'est pas dans la fenêtre, émet `CursorLeft`, et
  `cursor_position()` vaut `None`. Pour tester la souris, demander à Primo.

Capture de la zone client d'une fenêtre (fonctionne même si elle est recouverte) :

```powershell
param([int]$ProcessId, [string]$Out)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class WinShot {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
"@
[WinShot]::SetProcessDPIAware() | Out-Null
$h = (Get-Process -Id $ProcessId).MainWindowHandle
$r = New-Object WinShot+RECT; [WinShot]::GetClientRect($h, [ref]$r) | Out-Null
$bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp); $dc = $g.GetHdc()
[WinShot]::PrintWindow($h, $dc, 3) | Out-Null  # client only + GPU content
$g.ReleaseHdc($dc); $g.Dispose(); $bmp.Save($Out); $bmp.Dispose()
```

## Pièges connus

- **Temps de compilation** : la première compilation de Bevy prend plus de 20 minutes
  sous Windows (la cible wasm se compilera à part). Deux `cargo` lancés en même temps
  s'attendent sur le verrou de `target/`.
- **Unification des features** : `cargo build -p ta-client` et un build qui inclut
  `ta-server` (`--workspace`) ne résolvent pas les features des dépendances partagées de
  la même façon : le serveur (tokio, mio, tungstenite 0.30) ajoute des features à
  `windows-sys`, `getrandom`, `rand`, `futures-util` et `typenum`, qui sont tout en bas
  de l'arbre de Bevy. Le premier build de chaque variante recompile tout Bevy (26 min).
  Ensuite, cargo garde les deux variantes dans `target/` (le hash d'un artefact inclut
  ses features) et l'alternance ne coûte plus rien (vérifié). Ne pas vider `target/`
  sans raison. `resolver.feature-unification = "workspace"` réglerait le problème, mais
  cette option est encore instable (cargo 1.98 l'ignore sans `-Zfeature-unification`).
- **Bevy 0.19** : vérifier les API dans
  `~/.cargo/registry/src/index.crates.io-*/bevy_*-0.19.1/src/` plutôt que de se fier à
  sa mémoire. Déjà rencontré :
  - `insert_non_send` et `remove_non_send` n'existent que sur `World` et `App` (les
    `*_non_send_resource` sont dépréciés). Depuis un système, passer par
    `commands.queue(|world: &mut World| ...)`.
  - `DespawnOnExit(état)` (ex-`StateScoped`) ; `GlobalAmbientLight` est la ressource
    (`AmbientLight` est un composant de caméra) ; `TextFont::from_font_size(16.0)` ;
    `BorderColor::all(couleur)` ; `Node` porte `border`, `padding`, etc.
  - `Option<T>` marche pour tout `SystemParam` ; `Single<...>` saute le système si
    l'entité n'existe pas.
  - Le prélude exporte les traits de couleur (`Mix`, `Alpha`, `ColorToComponents`).
- **ewebsock** : `WsSender` n'est pas `Send` sur le web (il contient un
  `Rc<WebSocket>`), d'où la ressource non-send.
- **Déterminisme** : voir les règles dans architecture.md.
