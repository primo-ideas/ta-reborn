# Journal des décisions

Format : date, décision, raison, alternatives écartées. Ajouter en bas, ne pas
réécrire l'historique : une décision annulée reçoit une nouvelle entrée.

## 2026-09-25 : premières briques

1. **Bevy 0.19.1**, features par défaut. C'est la dernière version stable (la 0.20 est
   en RC). Je connais mal ses API récentes : je vérifie dans
   `~/.cargo/registry/src/*/bevy_*-0.19.1/` avant d'écrire du code (voir dev.md).
2. **Lockstep déterministe** plutôt qu'un serveur autoritaire avec snapshots.
   Raisons : dans un RTS à centaines d'unités, la bande passante dépend alors des
   commandes et non du nombre d'unités ; solo et en ligne partagent le même code ;
   les replays sont gratuits (liste des tours). C'est aussi le modèle de TA.
   Coût : simulation strictement déterministe, désyncs à surveiller (checksums).
3. **Tours cadencés par un serveur relais** plutôt qu'un pair-à-pair pur. Le
   navigateur n'a ni TCP ni UDP bruts ; un relais WebSocket marche partout ; un client
   lent ne bloque pas les autres, il rattrape son retard.
4. **WebSocket** : `ewebsock` côté client (même API en natif et en wasm),
   `tokio-tungstenite` côté serveur. WebRTC et WebTransport écartés pour l'instant
   (trop complexes pour un bénéfice faible avec du lockstep).
5. **postcard** pour sérialiser (compact, serde). `bincode` écarté : il n'est plus
   maintenu depuis 2025.
6. **Virgule fixe maison (Q32.32)** plutôt que des f32 : glam (SIMD ou non) et les
   fonctions transcendantes de libm ne donnent pas les mêmes bits en natif et en wasm.
   Maison plutôt que la crate `fixed` : le besoin est minuscule (add, sub, mul, div,
   `u128::isqrt`).
7. **Pas encore de trigonométrie dans la simulation** : l'orientation des unités est
   calculée par le client (en flottants, visuel seulement). À ajouter en arithmétique
   entière (table ou CORDIC) quand l'orientation aura un effet de jeu : vitesse de
   rotation, tourelles, tir.
8. **Carte procédurale** (depuis la seed) plutôt que des fichiers de carte : pas de
   pipeline d'assets pour l'instant. Solo : seed fixe (1). En ligne : seed tirée par le
   serveur.
9. **Interface en anglais.** La police par défaut de Bevy est un sous-ensemble de
   FiraMono ; la présence des accents n'a pas été vérifiée. Docs et échanges en français.
10. **Périmètre minimal**, conformément au `~/.claude/CLAUDE.md` de Primo : pas de
    combat, d'IA, d'économie ni de construction dans cette première itération. Le
    solo est un bac à sable avec un seul joueur.
11. **Web reporté** (demande de Primo : « on verra wasm plus tard »). Les choix faits
    pour le web restent en place (ewebsock, `Connection` non-send, pas de `SystemTime`
    côté client, `index.html`), mais rien n'a été compilé pour `wasm32`.
