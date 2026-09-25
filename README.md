# TA Reborn

Imitation de *Total Annihilation* en Rust avec [Bevy](https://bevy.org) : 3D, solo et
en ligne, natif et navigateur.

**État** : premières briques. Terrain procédural, chars, sélection, ordres de
déplacement, lockstep en solo et en ligne. Voir [docs/roadmap.md](docs/roadmap.md).

## Lancer

Prérequis : Rust stable ≥ 1.95. La première compilation de Bevy est longue.

```sh
cargo run -p ta-client            # menu : Solo / Online
cargo run -p ta-client -- solo    # directement en solo
```

En ligne (2 joueurs) :

```sh
cargo run -p ta-server                                  # relais sur 0.0.0.0:7878
cargo run -p ta-client -- online                        # dans deux terminaux
cargo run -p ta-client -- online --server ws://HOTE:7878
```

Navigateur (pas encore vérifié, prévu pour plus tard) :

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cd crates/ta-client && trunk serve                      # http://127.0.0.1:8080
```

Sur le web, le bouton Online se connecte à `ws://<hôte de la page>:7878`, ou à
l'adresse donnée par `?server=ws://HOTE:PORT`.

## En jeu

Clic gauche ou rectangle : sélection · clic droit : déplacer · `S` : stop ·
flèches : caméra · molette : zoom · Échap : menu.

## Organisation

| Dossier | Rôle |
|---|---|
| `crates/ta-sim` | Simulation déterministe (virgule fixe, sans Bevy) |
| `crates/ta-net` | Protocole client/serveur et ordonnancement des tours |
| `crates/ta-server` | Relais WebSocket : parties et tempo des tours |
| `crates/ta-client` | Client Bevy 3D (natif et wasm) |
| `docs/` | Notes de travail : architecture, décisions, roadmap |

Licence : domaine public ([Unlicense](LICENSE)).
