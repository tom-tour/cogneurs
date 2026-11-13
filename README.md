# Cogneurs

[![Support me on Ko-fi](https://img.shields.io/badge/Ko--fi-Support%20me-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/tom_t)

**Play:** https://tom-tour.github.io/cogneurs/ (online when the home server is running, offline solo otherwise)

Browser arena game where you don't fight: you write your fighter's **battle plan**, then watch it play out against other players' fighters.
Engine and server are written in Rust; the engine also runs in the page as WebAssembly.

- Each turn, the fighter plays the first rule of its plan whose condition holds and whose action is possible. Otherwise it attacks with whatever it holds: it never draws a weapon on its own.
- **No randomness in fights.** Every chance is a gauge: a 9% dodge chance adds 9 to the dodge gauge each time the fighter is attacked, and the dodge happens when the gauge reaches 100 (the overflow is kept: 99 + 5 leaves 4). Same for critical hits, blocks, ripostes, stuns and disarms. Rules can react to the gauges ("enemy dodge ready", "my crit ready").
- Stats come in opposed pairs (Strength / Toughness, Accuracy / Evasion, Critical / Composure, Control / Tenacity), plus Speed and Endurance: one attack point cancels exactly one defense point.
- **No randomness in progression either.** You spend your own points. Odd levels give +2 points and a choice: one more plan rule (3 at level 1, 8 at most) or a new condition. Even levels give a node of the skill tree: eight branches in opposed pairs (Power / Rampart, Finesse / Composure, Control / Tenacity, Rage / Survival), each node unlocked by the one before it. Everything can be redone for free between fights.
- **Presets.** A preset is what the fighter carries (3 weapons and 3 skills at most, out of everything it owns) plus a battle plan and a starting stance. You pick the attack preset before each fight; the defense preset is used when someone attacks you. "Take my best weapon" only looks at carried weapons.
- **Weapon mastery.** Every hit landed with a weapon trains it. At 10, 30 and 60 hits it gives +3% accuracy, then +10% damage, then +5% critical chance with that weapon. Mastery is kept on respec.
- **Synergies.** Two items carried together give a bonus (club + power strike, sword + precise strikes...). Each offensive synergy has a defensive answer that cancels it exactly; the in-game codex lists them all.
- You see the opponent's stats, everything it owns and its masteries, not what it carries nor its plan: both are revealed after the fight, with how many times each rule fired. Recent fights can be watched again from the history.
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

## Host from home

Double-click `Lancer-serveur.bat` (Windows). It installs cloudflared if needed, builds the game, starts the server and opens a free Cloudflare tunnel. The tunnel's `https://` address is published in a [gist](https://gist.github.com/tom-tour/a06e4d37f026c071419b6dcbec01d11a); the page on GitHub Pages reads it and plays online. Close the window and the page shows the server as offline (solo mode). Player data stays on your PC in `data/`.

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

- À chaque tour, le cogneur joue la première règle de son plan dont la condition est vraie et l'action possible. Sinon, il attaque avec ce qu'il a en main : il ne dégaine jamais tout seul.
- **Aucun hasard en combat.** Chaque chance est une jauge : 9 % d'esquive ajoutent 9 à la jauge d'esquive à chaque attaque reçue, et l'esquive tombe quand elle atteint 100 (le surplus est gardé : 99 + 5 donne 4). Pareil pour les critiques, blocages, ripostes, étourdissements et désarmements. Des règles peuvent réagir aux jauges (« sa prochaine esquive est prête », « mon critique est prêt »).
- Les caractéristiques vont par paires opposées (Force / Robustesse, Précision / Esquive, Critique / Sang-froid, Contrôle / Ténacité), plus Vitesse et Endurance : un point d'attaque annule exactement un point de défense.
- **Aucun hasard dans la progression non plus.** Tu répartis tes points toi-même. Niveaux impairs : +2 points et un choix entre une règle de plus dans le plan (3 au niveau 1, 8 au maximum) ou une nouvelle condition. Niveaux pairs : une case de l'arbre de compétences, huit branches en paires opposées (Puissance / Rempart, Finesse / Sang-froid, Contrôle / Ténacité, Rage / Survie), chaque case débloquée par celle d'avant. Tout se refait gratuitement entre deux combats.
- **Préréglages.** Un préréglage, c'est ce que le cogneur emporte (3 armes et 3 compétences au plus, parmi tout ce qu'il possède), plus un plan de bataille et une posture de départ. Tu choisis le préréglage d'attaque avant chaque combat ; celui de défense sert quand on t'attaque. « Prendre ma meilleure arme » ne regarde que les armes emportées.
- **Maîtrise des armes.** Chaque coup qui touche avec une arme l'entraîne. À 10, 30 et 60 coups, elle donne +3 % de précision, puis +10 % de dégâts, puis +5 % de critique avec cette arme. La maîtrise est gardée en cas de respécialisation.
- **Synergies.** Deux objets emportés ensemble donnent un bonus (massue + coup puissant, épée + coups précis…). Chaque synergie offensive a sa réponse défensive qui l'annule exactement ; le codex du jeu les liste toutes.
- Tu vois les caractéristiques de l'adversaire, tout ce qu'il possède et ses maîtrises, pas ce qu'il emporte ni son plan : les deux sont révélés après le combat, avec le nombre de fois où chaque règle a servi. Les derniers combats peuvent être revus depuis l'historique.
- En ligne, les adversaires sont les cogneurs des autres joueurs (complétés par des bots). Le serveur calcule chaque combat : impossible de tricher sur le résultat.

**Jouer :** https://tom-tour.github.io/cogneurs/ (en ligne quand le serveur maison tourne, solo hors ligne sinon)

Si le jeu te plaît, tu peux [me soutenir sur Ko-fi](https://ko-fi.com/tom_t).

## Lancer en local

```sh
sh build.sh
cargo run --release -p cogneurs-server
```

Puis ouvrir http://localhost:8137. Tests : `cargo test`.

## Héberger depuis chez soi

Double-clique sur `Lancer-serveur.bat` (Windows). Il installe cloudflared si besoin, compile le jeu, lance le serveur et ouvre un tunnel Cloudflare gratuit. L'adresse `https://` du tunnel est publiée dans un [gist](https://gist.github.com/tom-tour/a06e4d37f026c071419b6dcbec01d11a) ; la page GitHub Pages la lit et passe en ligne. Ferme la fenêtre et la page affiche le serveur hors ligne (mode solo). Les données des joueurs restent sur ton PC, dans `data/`.

## Jouer avec un ami

Le serveur sert aussi la page : ton ami n'a besoin que de son adresse.

- Même réseau : lance le serveur et donne-lui `http://<ton-ip-locale>:8137` (autorise le programme dans le pare-feu).
- Par internet : expose le serveur avec un tunnel, par exemple `cloudflared tunnel --url http://localhost:8137`, et partage l'adresse `https://` affichée.

## Hébergement

- **GitHub Pages** héberge seulement la page (`.github/workflows/pages.yml`). Sans serveur, elle tourne en solo hors ligne.
- **Le serveur** a besoin d'un hébergeur qui exécute des programmes : un VPS, ou un hébergeur de conteneurs avec le `Dockerfile`. Garder `/data` sur un volume persistant. Pour mettre la page sur Pages et le serveur ailleurs, indiquer l'adresse du serveur dans `web/config.js`.

## License

GPL-3.0-or-later.
