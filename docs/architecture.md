# Architecture

Imitation de Total Annihilation en Rust + Bevy : 3D, solo et en ligne, natif et navigateur.

## Crates

```
crates/
  ta-sim     simulation déterministe : pas de Bevy, pas de flottants
  ta-net     messages client/serveur (postcard) + Lockstep (ordonnanceur de tours)
  ta-server  relais WebSocket (tokio) : forme les parties et donne le tempo des tours
  ta-client  client Bevy 0.19 (natif ; web prévu mais jamais compilé, voir dev.md)
```

Dépendances : `ta-client → ta-net → ta-sim`, `ta-server → ta-net`.
Le serveur ne compile pas la logique de jeu : il ne simule rien.

## Modèle réseau : lockstep déterministe, tours cadencés par le serveur

- Chaque client simule toute la partie ; seules les **commandes** circulent.
- Le serveur regroupe les commandes reçues et diffuse un `Turn { number, commands }`
  toutes les `TURN_DURATION` (100 ms), même vide. L'ordre des commandes dans le tour
  est l'ordre d'arrivée au serveur, identique pour tous.
- Un tour = `TICKS_PER_TURN` (3) ticks à `TICK_RATE` (30 Hz, comme TA). Les commandes
  d'un tour sont appliquées au début de son premier tick.
- `ta_net::Lockstep` n'avance que sur les tours reçus (`buffered_ticks`). Dans
  `session::drive_simulation` : tampon vide → attente ; tampon > `MAX_BUFFERED_TICKS`
  → rattrapage (jusqu'à `MAX_TICKS_PER_FRAME` ticks par frame).
- **Solo = même chemin** : le client fabrique lui-même les tours à la demande à partir
  de `Session::pending`. La simulation ne sait pas si elle est en ligne.
- **Désync** : tous les `CHECKSUM_INTERVAL` tours, chaque client envoie `Sim::checksum()`.
  Le serveur compare et diffuse `Desync { turn }`, affiché dans le HUD.
- Latence d'un ordre en ligne : trajet vers le serveur + attente du prochain tour
  (≤ 100 ms) + trajet retour.

Flux d'une commande :

```
selection.rs (clic droit / S) → Session::issue
  solo   : Session::pending → Turn local ─────────────────────┐
  online : ClientMsg::Command → serveur → ServerMsg::Turn ─────┤ (tous les clients)
                                                              ▼
                   Lockstep::push_turn → Lockstep::step → Sim::apply puis Sim::step
```

### Serveur (`ta-server`)

- `serve(listener)` : une tâche tokio par connexion. Poignée de main
  `Join { version }` → `Waiting` (ou `VersionMismatch`).
- Lobby = file d'attente ; à `PLAYERS_PER_MATCH` (2) joueurs → `run_match`, une tâche
  par partie. Les connexions et la partie communiquent par canaux mpsc (`Seat`).
- Seed de la carte = horloge système. Déconnexion → `PlayerLeft` ; la partie
  s'arrête quand tous les joueurs sont partis.

## Déterminisme : règles pour `ta-sim`

- **Pas de f32/f64.** `Fx` = virgule fixe Q32.32 sur `i64` (mul/div via `i128`,
  longueur via `u128::isqrt`). `Fx::from_f32`/`to_f32` sont réservés au client : rendu,
  et conversion d'un clic en commande (l'arrondi est fait une seule fois, par
  l'émetteur, puis la valeur exacte voyage dans la commande).
- **Pas d'itération sur `HashMap`/`HashSet`.** Les unités sont dans un `Vec` trié par
  `UnitId`.
- **Aléatoire** : uniquement dérivé de la seed (hash SplitMix64 pour le terrain).
- **Débordements** : toujours `wrapping_*` dans les hash. Un débordement panique en
  debug et boucle en release, donc un client debug et un client release divergeraient.
  Le test du terrain a déjà trouvé ce bug.
- **Commandes validées** par la simulation : un joueur ne commande que ses unités.
  Les messages réseau ne sont pas fiables.
- `Sim::checksum` (FNV-1a) doit couvrir **tout** l'état : tout nouveau champ d'état
  doit y être ajouté.
- Changer les règles de simulation ou les messages ⇒ incrémenter `PROTOCOL_VERSION`.

## Simulation (`ta-sim`) aujourd'hui

- `Terrain` : grille carrée de 128 × 128 cellules de 2 m (256 m de côté), 4 octaves de
  bruit de valeur. La hauteur 0 est le niveau de l'eau, visuel seulement pour l'instant.
- `Unit` : `id`, `owner`, `pos` (`FxVec2`, plan x/z), `target` optionnelle. Un seul
  type (char), 6 m/s, pas d'orientation simulée.
- `Command` : `Move` (formation en grille centrée sur la cible, 4 m d'écart) et `Stop`.
- 1 ou 2 joueurs (`MAX_PLAYERS`), 6 unités chacun, départs à l'ouest et à l'est.

## Client (`ta-client`)

États `AppState::Menu` → `InGame` (Échap ramène au menu). Modules :

- `menu.rs` : boutons Solo et Online, connexion, lancement (`ta-client [solo|online]
  [--server URL]` en natif, `?server=` sur le web).
- `session.rs` : ressource `Session` (Lockstep, tours solo, interpolation entre ticks,
  message réseau affiché), `drive_simulation`, HUD, fin de partie.
- `net.rs` : `Connection` (ewebsock, messages binaires postcard). **Ressource non-send**
  car le `WsSender` web contient un `Rc` ; elle se crée donc via
  `commands.queue(|world| world.insert_non_send(..))`.
- `scene.rs` : maillage du terrain (couleur selon la hauteur), eau, lumière, modèles des
  unités (`UnitView`) interpolés entre deux ticks, orientation déduite du mouvement.
- `camera.rs` : caméra RTS (flèches, molette), inclinée d'environ 55°, regard vers le
  nord (-Z).
- `selection.rs` : clic ou rectangle de sélection (nœud UI), ordres (clic droit =
  `Move` avec raymarching sur le terrain, S = `Stop`), anneaux de sélection (gizmos).

Conventions : les axes x/z de la sim sont les axes X/Z de Bevy, Y vers le haut ;
l'avant des modèles est -Z ; l'UI est en anglais (voir decisions.md).
