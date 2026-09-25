# Feuille de route

## Fait

### 2026-09-25 : premières briques

- Workspace de 4 crates (`ta-sim`, `ta-net`, `ta-server`, `ta-client`).
- Simulation déterministe : virgule fixe Q32.32, terrain procédural, unités, ordres
  `Move` (en formation) et `Stop`, checksum.
- Lockstep : même chemin en solo (tours générés localement) et en ligne (tours du
  serveur), rattrapage et attente, détection de désync.
- Relais WebSocket : file d'attente, parties à 2, tours à 100 ms, `PlayerLeft`, contrôle
  de version.
- Client Bevy 3D : menu, terrain coloré selon la hauteur, eau, chars provisoires aux
  couleurs des joueurs, interpolation entre ticks, caméra RTS, sélection (clic ou
  rectangle), ordres, HUD. Natif seulement : le web est préparé mais pas vérifié.
- Tests : sim (10), protocole + lockstep (4), intégration du relais (2) ; `clippy` sans
  avertissement.
- Vérifié à l'écran (captures de la fenêtre) : solo natif ; en ligne avec un relais et
  deux clients (même chronomètre, pas de désync pendant plus d'une minute, « Player 2
  left » après fermeture d'un client).
- **Pas encore vérifié : la souris** (sélection au clic ou au rectangle, déplacement au
  clic droit, `S`). Impossible à simuler sans prendre la main sur le curseur de Primo :
  test manuel demandé.

## Reporté par Primo

- **Web (wasm)** : première compilation `wasm32`, bundle trunk, test de la connexion en
  ligne depuis un navigateur. Voir dev.md, section « Web ».

## Prochaines briques (ordre proposé, à valider avec Primo)

1. **Combat** : armes, projectiles, points de vie, destruction. Le client devra alors
   créer et supprimer les `UnitView` au fil de l'eau (aujourd'hui elles sont créées une
   seule fois, au début de la partie).
2. **Déplacement plus réaliste** : orientation simulée, vitesse de rotation,
   accélération (trigonométrie entière, voir decisions.md n° 7).
3. **IA pour le solo** : un joueur piloté par la simulation elle-même, donc déterministe,
   ce qui la rend aussi utilisable en ligne.
4. **Collisions entre unités**, puis **pathfinding** (grille, pentes, eau infranchissable).
5. **Économie à la TA** : métal et énergie, Commandant, construction de bâtiments et
   d'unités.
6. **Brouillard de guerre** et ligne de vue (grâce aux hauteurs du terrain).
7. **Lobby** : salons nommés, nombre de joueurs, reconnexion ; **replays** (enregistrer
   les tours).
8. **Assets** : vrais modèles 3D, sons, effets.

## Limitations connues

- Le solo est un bac à sable à un joueur : pas d'IA, pas de combat.
- Les unités se traversent et ignorent le relief ; l'eau est décorative (elles roulent
  à sa surface).
- En ligne : exactement 2 joueurs, une seule file d'attente, pas de reconnexion ; si un
  joueur part, l'autre continue seul.
- Pas de TLS (seulement `ws://`) : une page servie en `https://` ne pourra pas se
  connecter (le navigateur bloque le contenu mixte).
