# Cogneurs

[![Support me on Ko-fi](https://img.shields.io/badge/Ko--fi-Support%20me-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/tom_t)

**Play:** https://tom-tour.github.io/cogneurs/ (offline solo mode until a server is hosted)

Browser arena game where you don't fight: you write your fighter's **battle plan**, then watch it play out against other players' fighters.
Engine and server are written in Rust; the engine also runs in the page as WebAssembly.

- Each turn, the fighter plays the first rule of its plan whose condition holds and whose action is possible.
- **No randomness in fights.** Every chance is a gauge: a 9% dodge chance adds 9 to the dodge gauge each time the fighter is attacked, and the dodge happens when the gauge reaches 100 (the overflow is kept: 99 + 5 leaves 4). Same for critical hits, blocks, ripostes, stuns and disarms. Rules can react to the gauges ("enemy dodge ready", "my crit ready").
- At each level you pick 1 upgrade out of 3. Every 2 levels the plan gets one more rule.
- You see the opponent's gear, not its plan. "Prediction" plays your plan against 40 plans the opponent could have; the real plan is revealed after the fight.
- Online, opponents are other players' fighters (bots fill in). The server runs every fight, so results cannot be forged.

## Layout

- `engine/`: fight engine, progression, opponent generation (Rust, compiled to WebAssembly for the page).
- `server/`: HTTP server (Rust, axum): accounts, fights, leaderboard, JSON file storage.
- `web/`: the page. It plays online when the server answers, offline solo otherwise.

If you enjoy it, you can [support me on Ko-fi](https://ko-fi.com/tom_t).

## Run locally

Needs Rust with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`).

```sh
sh build.sh
cargo run --release -p cogneurs-server
```

Then open http://localhost:8137. Tests: `cargo test`.

Server settings (environment variables): `PORT` (8137), `COGNEURS_DATA` (`data/cogneurs.json`), `COGNEURS_WEB` (`web`), `COGNEURS_DAILY_FIGHTS` (20, 0 = unlimited).

## Play with a friend

The server also serves the page, so your friend only needs its address:

- Same network: run the server and give them `http://<your-local-ip>:8137` (allow the program through the firewall).
- Over the internet: expose the server with a tunnel, e.g. `cloudflared tunnel --url http://localhost:8137`, and share the `https://` address it prints.

## Hosting

- **GitHub Pages** hosts the page only (`.github/workflows/pages.yml`). Without a server it runs in offline solo mode.
- **The server** needs a host that runs programs: any VPS, or a container host using the `Dockerfile`. Keep `/data` on a persistent volume. To use Pages for the page and the server elsewhere, put the server address in `web/config.js`.

---

# Cogneurs (français)

Jeu d'arène dans le navigateur où tu ne te bats pas : tu écris le **plan de bataille** de ton cogneur, puis tu le regardes l'appliquer contre les cogneurs des autres joueurs.
Moteur et serveur en Rust ; le moteur tourne aussi dans la page en WebAssembly.

- À chaque tour, le cogneur joue la première règle de son plan dont la condition est vraie et l'action possible.
- **Aucun hasard en combat.** Chaque chance est une jauge : 9 % d'esquive ajoutent 9 à la jauge d'esquive à chaque attaque reçue, et l'esquive tombe quand elle atteint 100 (le surplus est gardé : 99 + 5 donne 4). Pareil pour les critiques, blocages, ripostes, étourdissements et désarmements. Des règles peuvent réagir aux jauges (« sa prochaine esquive est prête », « mon critique est prêt »).
- À chaque niveau tu choisis 1 amélioration parmi 3. Tous les 2 niveaux, le plan gagne une règle.
- Tu vois l'équipement de l'adversaire, pas son plan. Le « Pronostic » joue ton plan contre 40 plans qu'il pourrait avoir ; le vrai plan est révélé après le combat.
- En ligne, les adversaires sont les cogneurs des autres joueurs (complétés par des bots). Le serveur calcule chaque combat : impossible de tricher sur le résultat.

**Jouer :** https://tom-tour.github.io/cogneurs/ (solo hors ligne tant qu'aucun serveur n'est hébergé)

Si le jeu te plaît, tu peux [me soutenir sur Ko-fi](https://ko-fi.com/tom_t).

## Lancer en local

```sh
sh build.sh
cargo run --release -p cogneurs-server
```

Puis ouvrir http://localhost:8137. Tests : `cargo test`.

## Jouer avec un ami

Le serveur sert aussi la page : ton ami n'a besoin que de son adresse.

- Même réseau : lance le serveur et donne-lui `http://<ton-ip-locale>:8137` (autorise le programme dans le pare-feu).
- Par internet : expose le serveur avec un tunnel, par exemple `cloudflared tunnel --url http://localhost:8137`, et partage l'adresse `https://` affichée.

## Hébergement

- **GitHub Pages** héberge seulement la page (`.github/workflows/pages.yml`). Sans serveur, elle tourne en solo hors ligne.
- **Le serveur** a besoin d'un hébergeur qui exécute des programmes : un VPS, ou un hébergeur de conteneurs avec le `Dockerfile`. Garder `/data` sur un volume persistant. Pour mettre la page sur Pages et le serveur ailleurs, indiquer l'adresse du serveur dans `web/config.js`.

## License

GPL-3.0-or-later.
